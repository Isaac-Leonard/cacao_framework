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
    sub_views: Rc<RefCell<HashMap<usize, Box<dyn Layout>>>>,
    vdom: Rc<RefCell<HashMap<usize, VNode<T>>>>,
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
            for (_, comp) in self.vdom.borrow().iter() {
                if let VNode::Custom(comp) = comp {
                    comp.renderable.on_message(id)
                }
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

impl<T: Component + ?Sized> VNode<T> {
    pub fn as_button(&self) -> Option<&VButton<T>> {
        if let Self::Button(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_button_mut(&mut self) -> Option<&mut VButton<T>> {
        if let Self::Button(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_label(&self) -> Option<&VLabel> {
        if let Self::Label(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_label_mut(&mut self) -> Option<&mut VLabel> {
        if let Self::Label(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_custom(&self) -> Option<&VComponent> {
        if let Self::Custom(v) = self {
            Some(v)
        } else {
            None
        }
    }
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
    fn same_component_as(&self, other: &dyn Renderable) -> bool;
    fn update_props_from(&self, other: &dyn Renderable);
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
    fn same_component_as(&self, other: &dyn Renderable) -> bool {
        other.as_any().is::<Self>()
    }
    fn update_props_from(&self, other: &dyn Renderable) {
        self.update_props(
            other
                .as_any()
                .downcast_ref::<Self>()
                .unwrap()
                .props
                .borrow()
                .clone(),
        );
    }

    fn render(&self) {
        let vdom = T::render(&*self.props.borrow(), &*self.state.borrow());
        let keys_to_render = vdom.iter().map(|(key, _)| *key).collect::<Vec<_>>();
        let changes = vdom
            .into_iter()
            .flat_map(|(key, node)| {
                let vdom = self.vdom.borrow();
                let existing_component = vdom.get(&key);
                let changes = match existing_component {
                    Some(existing_component) => self.diff_nodes(existing_component, node),
                    None => vec![VDomDiff::InsertNode(node)],
                };
                changes
                    .into_iter()
                    .map(|change| (key, change))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        for (key, change) in changes {
            let mut sub_views = self.sub_views.borrow_mut();
            let mut vdom = self.vdom.borrow_mut();
            match change {
                VDomDiff::InsertNode(mut node) => {
                    let view = self.create_component(&mut node);
                    self.parent_view.add_subview(view.as_ref());
                    sub_views.insert(key, view);
                    vdom.insert(key, node);
                }
                VDomDiff::ReplaceWith(mut node) => {
                    vdom.remove(&key);
                    sub_views.remove(&key).unwrap().remove_from_superview();
                    let view = self.create_component(&mut node);
                    self.parent_view.add_subview(view.as_ref());
                    sub_views.insert(key, view);
                    vdom.insert(key, node);
                }
                VDomDiff::UpdateLabelText(text) => {
                    let node = vdom.get_mut(&key).unwrap();
                    let label = sub_views.get_mut(&key).unwrap();
                    label.downcast::<Label>().set_text(&text);
                    node.as_label_mut().unwrap().text = text;
                }
                VDomDiff::UpdateButtonText(text) => {
                    let node = vdom.get_mut(&key).unwrap();
                    let button = sub_views.get_mut(&key).unwrap();
                    button.downcast::<Button>().set_text(&text);
                    node.as_button_mut().unwrap().text = text;
                }
                VDomDiff::UpdateButtonClick(handler) => {
                    let node = vdom.get_mut(&key).unwrap();
                    let button = sub_views.get_mut(&key).unwrap();
                    node.as_button_mut().unwrap().click = handler;
                    if let Some(handler) = handler {
                        let id = gen_id();
                        self.handlers.borrow_mut().insert(id, handler);
                        button
                            .downcast::<Button>()
                            .set_action(move |_| App::<D, usize>::dispatch_main(id));
                    } else {
                        button.downcast::<Button>().set_action(|_| {});
            }
        }
                VDomDiff::UpdatePropsFrom(component) => {
                    let node = vdom.get_mut(&key).unwrap();
                    node.as_custom()
                        .unwrap()
                        .renderable
                        .as_ref()
                        .update_props_from(component.renderable.as_ref());
                }
            }
        }
        let mut vdom = self.vdom.borrow_mut();
        let keys_to_remove = vdom
            .keys()
            .filter(|key| !keys_to_render.contains(key))
            .copied()
            .collect::<Vec<_>>();
        let mut sub_views = self.sub_views.borrow_mut();
        for key in keys_to_remove {
            vdom.remove(&key);
            if let Some(x) = sub_views.remove(&key) {
                x.remove_from_superview()
            }
        }
        let views_to_render = keys_to_render
            .iter()
            .map(|key| sub_views.get(key).unwrap().as_ref())
            .collect::<Vec<_>>();
        LayoutConstraint::activate(&top_to_bottom(views_to_render, &self.parent_view, 8.));
    }

    fn get_parent_view(&self) -> &View {
        &self.parent_view
    }
    fn on_message(&self, id: &usize) {
        self.on_message(id)
    }
}

pub trait DowncastLayout {
    fn as_any(&mut self) -> &mut dyn Any;
    fn downcast<T: Layout + Any>(&mut self) -> &mut T;
}

impl DowncastLayout for Box<dyn Layout> {
    fn as_any(&mut self) -> &mut dyn Any {
        &mut *self
    }
    fn downcast<T: Layout + Any>(&mut self) -> &mut T {
        self.as_any().downcast_mut::<T>().unwrap()
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
