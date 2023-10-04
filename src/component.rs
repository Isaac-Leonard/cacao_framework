use std::{
    any::{type_name, Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
    sync::atomic,
};

use cacao::{
    appkit::{App, AppDelegate},
    button::Button,
    foundation::NSInteger,
    input::{TextField, TextFieldDelegate},
    layout::{Layout, LayoutConstraint},
    listview::ListView,
    notification_center::Dispatcher,
    objc::msg_send,
    select::Select,
    text::Label,
    view::{View, ViewDelegate},
};

use crate::{layout::top_to_bottom, list_view::MyListView};

pub struct ComponentWrapper<T: Component + PartialEq, D: Dispatcher<Message> + AppDelegate> {
    props: Rc<RefCell<T::Props>>,
    state: Rc<RefCell<T::State>>,
    click_handlers: Rc<RefCell<HashMap<usize, ClickHandler<T>>>>,
    change_handlers: Rc<RefCell<HashMap<usize, ChangeHandler<T>>>>,
    select_handlers: Rc<RefCell<HashMap<usize, SelectHandler<T>>>>,
    parent_view: View,
    sub_views: Rc<RefCell<HashMap<usize, CacaoComponent<T, D>>>>,
    vdom: Rc<RefCell<HashMap<usize, VNode<T>>>>,
    component: PhantomData<T>,
    app: PhantomData<D>,
}

pub trait Component {
    type Props: Clone + PartialEq;
    type State: Clone + PartialEq + Default;
    type Message: Clone + PartialEq = ();
    fn render(props: &Self::Props, state: &Self::State) -> Vec<(usize, VNode<Self>)>;
    fn on_message(_msg: &Self::Message, _props: &Self::Props, _state: &mut Self::State) -> bool {
        false
    }
}

impl ViewDelegate for &dyn Renderable {
    const NAME: &'static str = "custom_component";
    fn did_load(&mut self, view: cacao::view::View) {
        self.render();
        view.add_subview(self.get_parent_view());
        LayoutConstraint::activate(&top_to_bottom(vec![self.get_parent_view()], &view, 8.));
    }
}

impl<T, D> ViewDelegate for ComponentWrapper<T, D>
where
    T: Component + Clone + PartialEq + 'static,
    D: Dispatcher<Message> + AppDelegate + 'static,
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
    D: Dispatcher<Message> + AppDelegate + 'static,
{
    pub fn new(props: T::Props) -> Self {
        Self {
            parent_view: View::new(),
            sub_views: Rc::default(),
            props: Rc::new(RefCell::new(props)),
            state: Rc::default(),
            click_handlers: Rc::default(),
            change_handlers: Default::default(),
            select_handlers: Default::default(),
            vdom: Rc::default(),
            component: PhantomData,
            app: PhantomData,
        }
    }

    /// Call this to let your component register button clicks
    pub fn on_message(&self, message: &Message) {
        match &message.payload {
            Payload::Click => {
                if let Some(handler) = self.click_handlers.borrow_mut().get_mut(&message.id) {
                    handler(&*self.props.borrow(), &mut *self.state.borrow_mut());
                }
                // We need this check in a separate block to ensure the borrow of handler is dropped
                if self.has_handler_for(&message.id) {
                    self.render()
                } else {
                    for (_, comp) in self.vdom.borrow().iter() {
                        if let VNode::Custom(comp) = comp {
                            comp.renderable.on_message(message)
                        }
                    }
                }
            }
            Payload::Change(value) => {
                let rerender =
                    if let Some(handler) = self.change_handlers.borrow_mut().get_mut(&message.id) {
                        handler(
                            value.as_str(),
                            &*self.props.borrow(),
                            &mut *self.state.borrow_mut(),
                        )
                    } else {
                        false
                    };
                // We need this check in a separate block to ensure the borrow of handler is dropped
                if rerender {
                    self.render()
                } else {
                    for (_, comp) in self.vdom.borrow().iter() {
                        if let VNode::Custom(comp) = comp {
                            comp.renderable.on_message(message)
                        }
                    }
                }
            }
            Payload::Select(value) => {
                let rerender =
                    if let Some(handler) = self.select_handlers.borrow_mut().get_mut(&message.id) {
                        handler(*value, &*self.props.borrow(), &mut *self.state.borrow_mut())
                    } else {
                        false
                    };
                // We need this check in a separate block to ensure the borrow of handler is dropped
                if rerender {
                    self.render()
                } else {
                    for (_, comp) in self.vdom.borrow().iter() {
                        if let VNode::Custom(comp) = comp {
                            comp.renderable.on_message(message)
                        }
                    }
                }
            }
            Payload::Custom(inner_message) => {
                for (_, comp) in self.vdom.borrow().iter() {
                    if let VNode::Custom(comp) = comp {
                        comp.renderable.on_message(message)
                    }
                }
                let rerender =
                    if let Some(message) = inner_message.as_ref().downcast_ref::<T::Message>() {
                        T::on_message(
                            message,
                            &*self.props.borrow(),
                            &mut *self.state.borrow_mut(),
                        )
                    } else {
                        false
                    };
                if rerender {
                    self.render()
                }
            }
        }
    }

    fn has_handler_for(&self, id: &usize) -> bool {
        self.click_handlers.borrow().contains_key(id)
            || self.change_handlers.borrow().contains_key(id)
    }
    pub fn update_props(&self, props: T::Props) {
        *self.props.borrow_mut() = props;
        self.render();
    }

    pub fn create_component(&self, vnode: &mut VNode<T>) -> CacaoComponent<T, D> {
        match vnode {
            VNode::Custom(component) => {
                let view = View::new();
                component
                    .renderable
                    .as_ref()
                    .did_load(view.clone_as_handle());
                CacaoComponent::View(view)
            }
            VNode::Label(data) => {
                let label = Label::new();
                label.set_text(&data.text);
                CacaoComponent::Label(label)
            }
            VNode::Text(text) => {
                let label = Label::new();
                label.set_text(text);
                CacaoComponent::Label(label)
            }
            VNode::Button(button) => {
                let mut btn = Button::new(button.text.as_ref());
                if let Some(handler) = button.click {
                    let id = gen_id();
                    self.click_handlers.borrow_mut().insert(id, handler);
                    btn.set_action(move |_| App::<D, Message>::dispatch_main(Message::click(id)));
                }
                CacaoComponent::Button(btn)
            }
            VNode::Select(select) => {
                let mut select_view = Select::new();
                if let Some(handler) = select.select {
                    let id = gen_id();
                    self.select_handlers.borrow_mut().insert(id, handler);
                    select_view.set_action(move |sender| {
                        let index: NSInteger = unsafe { msg_send![sender, indexOfSelectedItem] };
                        App::<D, Message>::dispatch_main(Message::select(id, index as usize))
                    });
                }
                CacaoComponent::Select(select_view)
            }
            VNode::TextInput(text_input) => {
                let id = gen_id();
                let input = TextField::with(TextInput::new(id));
                input.set_text(&text_input.initial_value);
                if let Some(handler) = text_input.change {
                    self.change_handlers.borrow_mut().insert(id, handler);
                };
                CacaoComponent::TextField(input)
            }
            VNode::List(list) => {
                eprintln!("processing VList of {}", type_name::<T>());
                let list = MyListView::<T, D>::with(
                    list.count,
                    list.render,
                    self.props.clone(),
                    self.state.clone(),
                );
                CacaoComponent::List(list)
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
            (VNode::Text(a), VNode::Text(b)) => {
                if *a != b {
                    vec![VDomDiff::UpdatePureText(b)]
                } else {
                    Vec::new()
                }
            }
            (VNode::Button(a), VNode::Button(b)) => {
                let mut changes = Vec::new();
                if a.text != b.text {
                    changes.push(VDomDiff::UpdateButtonText(b.text))
                }
                if a.click != b.click {
                    changes.push(VDomDiff::UpdateButtonClick(b.click))
                }
                changes
            }
            (VNode::Custom(a), VNode::Custom(b)) => {
                if *a == b {
                    Vec::new()
                } else if a.renderable.same_component_as(b.renderable.as_ref()) {
                    vec![VDomDiff::UpdatePropsFrom(b)]
                } else {
                    // Both are custom components but different kinds so we must replace it
                    vec![VDomDiff::ReplaceWith(VNode::Custom(b))]
                }
            }
            (_, b) => vec![VDomDiff::ReplaceWith(b)],
        }
    }
}

#[derive(PartialEq)]
pub enum VNode<T: Component + ?Sized> {
    Label(VLabel),
    Button(VButton<T>),
    TextInput(VTextInput<T>),
    List(VList<T>),
    Select(VSelect<T>),
    Text(&'static str),
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

    pub fn as_text_input(&self) -> Option<&VTextInput<T>> {
        if let Self::TextInput(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_text_input_mut(&mut self) -> Option<&mut VTextInput<T>> {
        if let Self::TextInput(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_text(&self) -> Option<&&'static str> {
        if let Self::Text(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_text_mut(&mut self) -> Option<&mut &'static str> {
        if let Self::Text(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_select(&self) -> Option<&VSelect<T>> {
        if let Self::Select(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_select_mut(&mut self) -> Option<&mut VSelect<T>> {
        if let Self::Select(v) = self {
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

#[derive(Clone, PartialEq)]
pub struct VTextInput<T: Component + ?Sized> {
    pub change: Option<ChangeHandler<T>>,
    pub initial_value: String,
}

#[derive(Clone, PartialEq)]
pub struct VList<T: Component + ?Sized> {
    pub count: usize,
    pub render: fn(index: usize, &T::Props, &T::State) -> Vec<VNode<T>>,
}

#[derive(PartialEq, Clone)]
pub struct VSelect<T: Component + ?Sized> {
    options: Vec<String>,
    select: Option<SelectHandler<T>>,
}

pub struct VComponent {
    pub type_id: TypeId,
    pub renderable: Box<dyn Renderable>,
}

impl VComponent {
    pub fn new<T, D>(props: T::Props) -> Self
    where
        T: Component + Clone + PartialEq + 'static,
        D: AppDelegate + Dispatcher<Message> + 'static,
    {
        Self {
            type_id: TypeId::of::<T>(),
            renderable: Box::new(ComponentWrapper::<T, D>::new(props)),
        }
    }
}

impl PartialEq for VComponent {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && self.renderable.equal_to(other.renderable.as_ref())
    }
}

type ClickHandler<T> = fn(&<T as Component>::Props, &mut <T as Component>::State);
type ChangeHandler<T> = fn(&str, &<T as Component>::Props, &mut <T as Component>::State) -> bool;
type SelectHandler<T> = fn(usize, &<T as Component>::Props, &mut <T as Component>::State) -> bool;

pub trait Renderable {
    fn copy(&self) -> Box<dyn Renderable>;
    fn as_any(&self) -> &dyn Any;
    fn equal_to(&self, other: &dyn Renderable) -> bool;
    fn same_component_as(&self, other: &dyn Renderable) -> bool;
    fn update_props_from(&self, other: Box<dyn Renderable>);
    fn render(&self);
    fn get_parent_view(&self) -> &View;
    fn on_message(&self, message: &Message);
}

impl<
        T: Component + PartialEq + Clone + 'static,
        D: AppDelegate + Dispatcher<Message> + 'static,
    > Renderable for ComponentWrapper<T, D>
{
    fn copy(&self) -> Box<dyn Renderable> {
        let wrapper: ComponentWrapper<T, D> = ComponentWrapper {
            props: Rc::clone(&self.props),
            state: Rc::clone(&self.state),
            click_handlers: Rc::clone(&self.click_handlers),
            change_handlers: Rc::clone(&self.change_handlers),
            select_handlers: Rc::clone(&self.select_handlers),
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
    fn update_props_from(&self, other: Box<dyn Renderable>) {
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
                    self.parent_view.add_subview(view.as_layout());
                    sub_views.insert(key, view);
                    vdom.insert(key, node);
                }
                VDomDiff::ReplaceWith(mut node) => {
                    vdom.remove(&key);
                    sub_views
                        .remove(&key)
                        .unwrap()
                        .as_layout()
                        .remove_from_superview();
                    let view = self.create_component(&mut node);
                    self.parent_view.add_subview(view.as_layout());
                    sub_views.insert(key, view);
                    vdom.insert(key, node);
                }
                VDomDiff::UpdateLabelText(text) => {
                    let node = vdom.get_mut(&key).unwrap();
                    let label = sub_views.get_mut(&key).unwrap();
                    label.as_label().unwrap().set_text(&text);
                    node.as_label_mut().unwrap().text = text;
                }
                VDomDiff::UpdatePureText(text) => {
                    let node = vdom.get_mut(&key).unwrap();
                    let label = sub_views.get_mut(&key).unwrap();
                    label.as_label().unwrap().set_text(text);
                    *node.as_text_mut().unwrap() = text;
                }
                VDomDiff::UpdateButtonText(text) => {
                    let node = vdom.get_mut(&key).unwrap();
                    let button = sub_views.get_mut(&key).unwrap();
                    button.as_button_mut().unwrap().set_text(&text);
                    node.as_button_mut().unwrap().text = text;
                }
                VDomDiff::UpdateButtonClick(handler) => {
                    let node = vdom.get_mut(&key).unwrap();
                    let button = sub_views.get_mut(&key).unwrap();
                    node.as_button_mut().unwrap().click = handler;
                    if let Some(handler) = handler {
                        let id = gen_id();
                        self.click_handlers.borrow_mut().insert(id, handler);
                        button.as_button_mut().unwrap().set_action(move |_| {
                            App::<D, Message>::dispatch_main(Message::click(id))
                        });
                    } else {
                        button.as_button_mut().unwrap().set_action(|_| {});
                    }
                }
                VDomDiff::UpdateInputChange(handler) => {
                    let node = vdom.get_mut(&key).unwrap();
                    let input = sub_views.get_mut(&key).unwrap();
                    node.as_text_input_mut().unwrap().change = handler;
                    let id = gen_id();
                    input
                        .as_text_field_mut()
                        .unwrap()
                        .delegate
                        .as_mut()
                        .unwrap()
                        .id = id;
                    if let Some(handler) = handler {
                        self.change_handlers.borrow_mut().insert(id, handler);
                    }
                }
                VDomDiff::UpdatePropsFrom(component) => {
                    let node = vdom.get_mut(&key).unwrap();
                    node.as_custom()
                        .unwrap()
                        .renderable
                        .as_ref()
                        .update_props_from(component.renderable);
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
                x.as_layout().remove_from_superview()
            }
        }
        let views_to_render = keys_to_render
            .iter()
            .map(|key| sub_views.get(key).unwrap().as_layout())
            .collect::<Vec<_>>();
        LayoutConstraint::activate(&top_to_bottom(views_to_render, &self.parent_view, 8.));
    }

    fn get_parent_view(&self) -> &View {
        &self.parent_view
    }
    fn on_message(&self, message: &Message) {
        self.on_message(message)
    }
}

pub enum CacaoComponent<T: Component + PartialEq, D: AppDelegate + Dispatcher<Message>> {
    Label(Label),
    Button(Button),
    View(View),
    TextField(TextField<TextInput<D>>),
    List(ListView<MyListView<T, D>>),
    Select(Select),
}

impl<T: Component + Clone + PartialEq, D: AppDelegate + Dispatcher<Message>> CacaoComponent<T, D> {
    pub fn as_label(&self) -> Option<&Label> {
        if let Self::Label(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_label_mut(&mut self) -> Option<&mut Label> {
        if let Self::Label(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_button(&self) -> Option<&Button> {
        if let Self::Button(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_button_mut(&mut self) -> Option<&mut Button> {
        if let Self::Button(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_view(&self) -> Option<&View> {
        if let Self::View(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_view_mut(&mut self) -> Option<&mut View> {
        if let Self::View(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_layout(&self) -> &dyn Layout {
        match self {
            CacaoComponent::Label(label) => label,
            CacaoComponent::Button(button) => button,
            CacaoComponent::View(view) => view,
            CacaoComponent::TextField(text_input) => text_input,
            CacaoComponent::List(list) => list,
            CacaoComponent::Select(select) => select,
        }
    }

    pub fn as_text_field(&self) -> Option<&TextField<TextInput<D>>> {
        if let Self::TextField(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_text_field_mut(&mut self) -> Option<&mut TextField<TextInput<D>>> {
        if let Self::TextField(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

fn gen_id() -> usize {
    static COUNTER: atomic::AtomicUsize = atomic::AtomicUsize::new(0);
    COUNTER.fetch_add(1, atomic::Ordering::SeqCst)
}

pub enum VDomDiff<T: Component> {
    UpdatePureText(&'static str),
    UpdateLabelText(String),
    UpdateButtonText(String),
    UpdateButtonClick(Option<ClickHandler<T>>),
    UpdateInputChange(Option<ChangeHandler<T>>),
    UpdatePropsFrom(VComponent),
    InsertNode(VNode<T>),
    ReplaceWith(VNode<T>),
}

pub struct TextInput<App: AppDelegate> {
    id: usize,
    app: PhantomData<App>,
}

impl<App: AppDelegate> TextInput<App> {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            app: PhantomData,
        }
    }
}

impl<D: AppDelegate + Dispatcher<Message>> TextFieldDelegate for TextInput<D> {
    const NAME: &'static str = "TextInput";
    fn text_did_change(&self, value: &str) {
        App::<D, Message>::dispatch_main(Message::change(self.id, value.to_owned()));
    }
}

#[derive(PartialEq, Debug)]
pub struct Message {
    pub id: usize,
    pub payload: Payload,
}

#[derive(Debug)]
pub enum Payload {
    Click,
    Change(String),
    Select(usize),
    Custom(Box<dyn Any + Send + Sync>),
}

impl Message {
    fn click(id: usize) -> Self {
        Self {
            id,
            payload: Payload::Click,
        }
    }
    fn change(id: usize, value: String) -> Self {
        Self {
            id,
            payload: Payload::Change(value),
        }
    }
    fn select(id: usize, value: usize) -> Self {
        Self {
            id,
            payload: Payload::Select(value),
        }
    }

    pub fn custom(message: impl Any + Send + Sync) -> Self {
        Self {
            // This is a bit silly but for now it needs an id and we don't want one that  will conflict with something else
            id: gen_id(),
            payload: Payload::Custom(Box::new(message)),
        }
    }
}

/// Take note that this will flatly return false for custom types
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Click, Self::Click) => true,
            (Self::Change(a), Self::Change(b)) => a == b,
            (Self::Custom(_), Self::Custom(_)) => false,
            _ => false,
        }
    }
}
