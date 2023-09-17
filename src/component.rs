use std::{cell::RefCell, collections::HashMap, marker::PhantomData, sync::atomic};

use cacao::{
    appkit::{App, AppDelegate},
    button::Button,
    layout::{Layout, LayoutConstraint},
    notification_center::Dispatcher,
    text::Label,
    view::{View, ViewDelegate},
};

use crate::layout::top_to_bottom;

pub struct Component<
    P: Clone + PartialEq,
    S: Clone + PartialEq + Default,
    T: Renderable<Props = P, State = S>,
    D: Dispatcher<usize> + AppDelegate,
> {
    view: View,
    sub_views: RefCell<Vec<Box<dyn Layout>>>,
    props: RefCell<P>,
    state: RefCell<S>,
    handlers: RefCell<HashMap<usize, fn(&P, &mut S)>>,
    vdom: RefCell<Vec<Discripter<P, S>>>,
    component: PhantomData<T>,
    app: PhantomData<D>,
}

pub trait Renderable {
    type Props: Clone + PartialEq;
    type State: Clone + PartialEq + Default;

    fn render(
        props: &Self::Props,
        state: &Self::State,
    ) -> Vec<Discripter<Self::Props, Self::State>>;
}

impl<P, S, T, D> ViewDelegate for Component<P, S, T, D>
where
    P: Clone + PartialEq,
    S: Clone + PartialEq + Default,
    T: Renderable<Props = P, State = S>,
    D: Dispatcher<usize> + AppDelegate,
{
    const NAME: &'static str = "custom_component";
    fn did_load(&mut self, view: cacao::view::View) {
        self.render();
        view.add_subview(&self.view);
    }
}

// The clone and PartialEq requirements here are needed by the compiler despite never being called on S as parts of the virtual DOM do get cloned
impl<P, S, T, D> Component<P, S, T, D>
where
    P: Clone + PartialEq,
    S: Clone + PartialEq + Default,
    T: Renderable<Props = P, State = S>,
    D: Dispatcher<usize> + AppDelegate,
{
    pub fn new(props: P) -> Self {
        Self {
            view: View::new(),
            sub_views: Vec::new().into(),
            props: RefCell::new(props),
            state: RefCell::default(),
            handlers: RefCell::default(),
            vdom: RefCell::default(),
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
        }
    }

    pub fn update_props(&self, props: P) {
        *self.props.borrow_mut() = props;
        self.render();
    }

    fn render(&self) {
        static COUNTER: atomic::AtomicUsize = atomic::AtomicUsize::new(0);
        let mut button_handlers = self.handlers.borrow_mut();
        let vdom = T::render(&*self.props.borrow(), &*self.state.borrow());
        let mut last_vdom = self.vdom.borrow_mut();
        if *last_vdom == vdom {
            return;
        }
        let vdom_len = vdom.len();
        let mut sub_views_ptr = self.sub_views.borrow_mut();
        for (i, component) in vdom.into_iter().enumerate() {
            if last_vdom.len() <= i || last_vdom[i] != component {
                last_vdom.insert(i, component.clone());
                let new_component = match component.kind {
                    ComponentType::Label => {
                        let label = Label::new();
                        label.set_text(component.text);
                        self.view.add_subview(&label);
                        Box::new(label) as Box<dyn Layout>
                    }
                    ComponentType::Button(handler) => {
                        let mut btn = Button::new(component.text.as_ref());
                        if let Some(handler) = handler {
                            let id = COUNTER.fetch_add(1, atomic::Ordering::SeqCst);
                            button_handlers.insert(id, handler);
                            btn.set_action(move |_| App::<D, usize>::dispatch_main(id));
                        }
                        self.view.add_subview(&btn);
                        Box::new(btn) as Box<dyn Layout>
                    }
                };
                self.view.add_subview(new_component.as_ref());
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
            &self.view,
            8.,
        ));
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Discripter<P: PartialEq + Clone, S: PartialEq + Clone + Default> {
    pub kind: ComponentType<P, S>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentType<P, S> {
    Label,
    Button(ClickHandler<P, S>),
}

type ClickHandler<P, S> = Option<fn(&P, &mut S)>;
