use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use cacao::appkit::AppDelegate;
use cacao::layout::{Layout, LayoutConstraint};
use cacao::listview::{ListView, ListViewDelegate};
use cacao::notification_center::Dispatcher;
use cacao::view::{View, ViewDelegate};

use crate::layout::top_to_bottom;
use crate::{CacaoComponent, Component, ComponentWrapper, Message, VNode};

/// A generic list view
pub struct MyListView<T: Component, D: Dispatcher<Message> + AppDelegate> {
    view: Option<ListView>,
    count: usize,
    render: fn(usize, &T::Props, &T::State) -> Vec<VNode<T>>,
    props: Rc<RefCell<T::Props>>,
    state: Rc<RefCell<T::State>>,
    app: PhantomData<D>,
    component: PhantomData<T>,
}

impl<T, D> MyListView<T, D>
where
    T: Component + Clone + PartialEq + 'static,
    D: Dispatcher<Message> + AppDelegate + 'static,
{
    pub fn new(
        count: usize,
        render: fn(usize, &T::Props, &T::State) -> Vec<VNode<T>>,
        props: Rc<RefCell<T::Props>>,
        state: Rc<RefCell<T::State>>,
    ) -> Self {
        Self {
            view: None,
            count,
            render,
            props,
            state,
            app: PhantomData,
            component: PhantomData,
        }
    }

    /// Not a good name
    pub fn with(
        count: usize,
        render: fn(usize, &T::Props, &T::State) -> Vec<VNode<T>>,
        props: Rc<RefCell<T::Props>>,
        state: Rc<RefCell<T::State>>,
    ) -> ListView<Self> {
        ListView::with(Self::new(count, render, props, state))
    }
}

impl<T, D> ListViewDelegate for MyListView<T, D>
where
    T: Component + Clone + PartialEq + 'static,
    D: Dispatcher<Message> + AppDelegate + 'static,
{
    const NAME: &'static str = "ThisIsIgnored";
    fn subclass_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    /// Essential configuration and retaining of a `ListView` handle to do updates later on.
    fn did_load(&mut self, view: ListView) {
        view.register(std::any::type_name::<T>(), Row::<T, D>::new);
        view.set_row_height(64.);
        LayoutConstraint::activate(&[
            view.height.constraint_equal_to_constant(100.0),
            view.width.constraint_equal_to_constant(100.0),
        ]);
        self.view = Some(view);
    }

    fn number_of_items(&self) -> usize {
        self.count
    }

    /// For a given row, dequeues a view from the system and passes the appropriate `Transfer` for
    /// configuration.
    fn item_for(&self, row: usize) -> cacao::listview::ListViewRow {
        let mut view = self
            .view
            .as_ref()
            .unwrap()
            .dequeue::<Row<T, D>>(std::any::type_name::<T>());
        if let Some(view) = &mut view.delegate {
            view.as_mut().configure_with(
                self.render,
                row,
                &*self.props.borrow(),
                &*self.state.borrow(),
            );
        }

        view.into_row()
    }
}

pub struct Row<T: Component + Clone + PartialEq, D: Dispatcher<Message> + AppDelegate> {
    view: View,
    sub_views: Vec<CacaoComponent<T, D>>,
    component: PhantomData<T>,
    app: PhantomData<D>,
}

impl<
        T: Component + Clone + PartialEq + 'static,
        D: Dispatcher<Message> + AppDelegate + 'static,
    > Row<T, D>
{
    pub fn new() -> Self {
        Self {
            view: View::new(),
            sub_views: Vec::new(),
            component: PhantomData,
            app: PhantomData,
        }
    }

    fn configure_with(
        &mut self,
        render: fn(usize, &T::Props, &T::State) -> Vec<VNode<T>>,
        index: usize,
        props: &T::Props,
        state: &T::State,
    ) {
        let mut vdom = render(index, props, state);
        for view in &self.sub_views {
            view.as_layout().remove_from_superview();
        }
        // Sshhh bit of a hack but it works
        // TODO: Try make it work better in the future
        let comp = ComponentWrapper::<T, D>::new(props.clone());
        self.sub_views = vdom
            .iter_mut()
            .map(|node| comp.create_component(node))
            .collect();
        for view in &self.sub_views {
            self.view.add_subview(view.as_layout())
        }
        LayoutConstraint::activate(&top_to_bottom(
            self.sub_views.iter().map(|view| view.as_layout()).collect(),
            &self.view,
            8.,
        ));
    }
}

impl<T: Component + Clone + PartialEq, D: Dispatcher<Message> + AppDelegate> ViewDelegate
    for Row<T, D>
{
    const NAME: &'static str = "Row";
    fn did_load(&mut self, view: View) {
        view.add_subview(&self.view);
        LayoutConstraint::activate(&[
            self.view.top.constraint_equal_to(&view.top),
            self.view.leading.constraint_equal_to(&view.leading),
            self.view.trailing.constraint_equal_to(&view.trailing),
            self.view.bottom.constraint_equal_to(&view.bottom),
        ]);
    }
}
