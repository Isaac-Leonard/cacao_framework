use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
    sync::atomic,
};

use cacao::{
    appkit::{App, AppDelegate},
    button::Button,
    layout::{Layout, LayoutConstraint},
    notification_center::Dispatcher,
    text::Label,
    view::{View, ViewDelegate},
};

use crate::layout::top_to_bottom;

pub struct ComponentWrapper<T: Component + Clone + PartialEq, D: Dispatcher<usize> + AppDelegate> {
    props: Rc<RefCell<T::Props>>,
    state: Rc<RefCell<T::State>>,
    handlers: Rc<RefCell<HashMap<usize, ClickHandler<T>>>>,
    parent_view: View,
    sub_views: Rc<RefCell<Vec<Box<dyn Layout>>>>,
    vdom: Rc<RefCell<Vec<VNode<T>>>>,
    pub sub_components: Rc<RefCell<Vec<Box<dyn Renderable>>>>,
    component: PhantomData<T>,
    app: PhantomData<D>,
}

pub trait Component {
    type Props: Clone + PartialEq;
    type State: Clone + PartialEq + Default;

    fn render(props: &Self::Props, state: &Self::State) -> Vec<(usize, VNode<Self>)>;
}

impl ViewDelegate for &dyn Renderable {
    const NAME: &'static str = "custom_component";
    fn did_load(&mut self, view: cacao::view::View) {
        self.render();
        view.add_subview(self.get_parent_view());
    }
}

impl<T, D> ViewDelegate for ComponentWrapper<T, D>
where
    T: Component + Clone + PartialEq + 'static,
    D: Dispatcher<usize> + AppDelegate + 'static,
{
    const NAME: &'static str = "custom_component";
    fn did_load(&mut self, view: cacao::view::View) {
        self.render();
        view.add_subview(self.get_parent_view());
    }
}

// The clone and PartialEq requirements here are needed by the compiler despite never being called on S as parts of the virtual DOM do get cloned
impl<T, D> ComponentWrapper<T, D>
where
    T: Component + PartialEq + Clone + 'static,
    D: Dispatcher<usize> + AppDelegate + 'static,
{
    pub fn new(props: T::Props) -> Self {
        Self {
            parent_view: View::new(),
            sub_views: Rc::default(),
            props: Rc::new(RefCell::new(props)),
            state: Rc::default(),
            handlers: Rc::default(),
            vdom: Rc::default(),
            sub_components: Rc::default(),
            component: PhantomData,
            app: PhantomData,
        }
    }

    /// Call this to let your component register button clicks
    pub fn on_message(&self, id: &usize) {
        if let Some(handler) = self.handlers.borrow_mut().get_mut(id) {
            handler(&*self.props.borrow(), &mut *self.state.borrow_mut());
        }
        // We need this check in a separate block to ensure the borrow of handler is dropped
        if self.handlers.borrow().contains_key(id) {
            self.render()
        } else {
            for comp in self.sub_components.borrow().iter() {
                comp.on_message(id)
            }
        }
    }

    pub fn update_props(&self, props: T::Props) {
        *self.props.borrow_mut() = props;
        self.render();
    }

    fn create_component(&self, vnode: &mut VNode<T>) -> Box<dyn Layout> {
        match vnode {
            VNode::Custom(component) => {
                let view = View::new();
                component
                    .renderable
                    .as_ref()
                    .did_load(view.clone_as_handle());
                Box::new(view) as Box<dyn Layout>
            }
            VNode::Label(data) => {
                let label = Label::new();
                label.set_text(&data.text);
                Box::new(label) as Box<dyn Layout>
            }
            VNode::Button(button) => {
                let mut btn = Button::new(button.text.as_ref());
                if let Some(handler) = button.click {
                    let id = gen_id();
                    self.handlers.borrow_mut().insert(id, handler);
                    btn.set_action(move |_| App::<D, usize>::dispatch_main(id));
                }
                Box::new(btn) as Box<dyn Layout>
            }
        }
    }

    fn diff_nodes(&self, a: &VNode<T>, b: VNode<T>) -> Vec<VDomDiff<T>> {
        match (a, b) {
            (VNode::Label(a), VNode::Label(b)) => {
                if a.text != b.text {
                    vec![VDomDiff::UpdateLabelText(b.text)]
                } else {
                    Vec::new()
                }
            }
            (_, b) => vec![VDomDiff::ReplaceWith(b)],
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum VNode<T: Component + ?Sized> {
    Label(VLabel),
    Button(VButton<T>),
    Custom(VComponent),
}

#[derive(Clone, PartialEq)]
pub struct VLabel {
    pub text: String,
}

#[derive(Clone, PartialEq)]
pub struct VButton<T: Component + ?Sized> {
    pub click: Option<ClickHandler<T>>,
    pub text: String,
}

pub struct VComponent {
    pub type_id: TypeId,
    pub renderable: Box<dyn Renderable>,
}

impl VComponent {
    pub fn new<T, D>(props: T::Props) -> Self
    where
        T: Component + Clone + PartialEq + 'static,
        D: AppDelegate + Dispatcher<usize> + 'static,
    {
        Self {
            type_id: TypeId::of::<T>(),
            renderable: Box::new(ComponentWrapper::<T, D>::new(props)),
        }
    }
}

impl Clone for VComponent {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            renderable: self.renderable.copy(),
        }
    }
}

impl PartialEq for VComponent {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && self.renderable.equal_to(other.renderable.as_ref())
    }
}

type ClickHandler<T> = fn(&<T as Component>::Props, &mut <T as Component>::State);

pub trait Renderable {
    fn copy(&self) -> Box<dyn Renderable>;
    fn as_any(&self) -> &dyn Any;
    fn equal_to(&self, other: &dyn Renderable) -> bool;
    fn render(&self);
    fn get_parent_view(&self) -> &View;
    fn on_message(&self, id: &usize);
}

impl<T: Component + PartialEq + Clone + 'static, D: AppDelegate + Dispatcher<usize> + 'static>
    Renderable for ComponentWrapper<T, D>
{
    fn copy(&self) -> Box<dyn Renderable> {
        let wrapper: ComponentWrapper<T, D> = ComponentWrapper {
            props: Rc::clone(&self.props),
            state: Rc::clone(&self.state),
            handlers: Rc::clone(&self.handlers),
            vdom: Rc::clone(&self.vdom),
            sub_views: Rc::clone(&self.sub_views),
            parent_view: self.parent_view.clone_as_handle(),
            sub_components: Rc::clone(&self.sub_components),
            component: PhantomData,
            app: PhantomData,
        };
        Box::new(wrapper)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equal_to(&self, other: &dyn Renderable) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .map(|rhs| self.props == rhs.props)
            .unwrap_or(false)
    }

    fn render(&self) {
        let mut button_handlers = self.handlers.borrow_mut();
        let mut sub_views_ptr = self.sub_views.borrow_mut();
        let vdom = T::render(&*self.props.borrow(), &*self.state.borrow());
        let vdom_len = vdom.len();
        let mut last_vdom = self.vdom.borrow_mut();
        for (i, (_key, component)) in vdom.into_iter().enumerate() {
            if last_vdom.len() <= i || last_vdom[i] != component {
                last_vdom.insert(i, component.clone());
                let new_component = match component {
                    VNode::Custom(component) => {
                        let mut sub_components = self.sub_components.borrow_mut();
                        sub_components.push(component.renderable);
                        let view = View::new();
                        sub_components
                            .last()
                            .unwrap()
                            .as_ref()
                            .did_load(view.clone_as_handle());
                        self.parent_view.add_subview(&view);
                        Box::new(view) as Box<dyn Layout>
                    }
                    VNode::Label(data) => {
                        let label = Label::new();
                        label.set_text(data.text);
                        self.parent_view.add_subview(&label);
                        Box::new(label) as Box<dyn Layout>
                    }
                    VNode::Button(button) => {
                        let mut btn = Button::new(button.text.as_ref());
                        if let Some(handler) = button.click {
                            let id = gen_id();
                            button_handlers.insert(id, handler);
                            btn.set_action(move |_| App::<D, usize>::dispatch_main(id));
                        }
                        self.parent_view.add_subview(&btn);
                        Box::new(btn) as Box<dyn Layout>
                    }
                };
                self.parent_view.add_subview(new_component.as_ref());
                sub_views_ptr.insert(i, new_component);
            }
        }
        last_vdom.truncate(vdom_len);
        sub_views_ptr
            .iter()
            .skip(vdom_len)
            .for_each(|view| view.remove_from_superview());
        sub_views_ptr.truncate(vdom_len);
        LayoutConstraint::activate(&top_to_bottom(
            sub_views_ptr.iter().map(|view| view.as_ref()).collect(),
            &self.parent_view,
            8.,
        ));
    }

    fn get_parent_view(&self) -> &View {
        &self.parent_view
    }
    fn on_message(&self, id: &usize) {
        self.on_message(id)
    }
}

pub trait DowncastLayout {
    fn as_any(&mut self) -> &dyn Any;
    fn downcast<T: Layout + Any>(&mut self) -> &T;
}

impl DowncastLayout for Box<dyn Layout> {
    fn as_any(&mut self) -> &dyn Any {
        &*self
    }
    fn downcast<T: Layout + Any>(&mut self) -> &T {
        self.as_any().downcast_ref::<T>().unwrap()
    }
}

fn gen_id() -> usize {
    static COUNTER: atomic::AtomicUsize = atomic::AtomicUsize::new(0);
    COUNTER.fetch_add(1, atomic::Ordering::SeqCst)
}

pub enum VDomDiff<T: Component> {
    UpdateLabelText(String),
    UpdateButtonText(String),
    UpdateButtonClick(Option<ClickHandler<T>>),
    UpdatePropsFrom(VComponent),
    InsertNode(VNode<T>),
    ReplaceWith(VNode<T>),
}
