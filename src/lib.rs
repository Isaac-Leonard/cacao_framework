mod component;
mod layout;
pub use component::*;

#[cfg(test)]
mod tests {
    /// Demonstrates the implementation of a simple counter component
    use super::*;

    #[derive(PartialEq, Clone)]
    pub struct CustomComponent;

    impl Component for CustomComponent {
        type Props = ();
        type State = u32;
        fn render(_props: &Self::Props, state: &Self::State) -> Vec<(usize, VNode<Self>)> {
            vec![
                (
                    0,
                VNode::Button(VButton {
                    click: Some(|_, state| *state += 1),
                    text: "Increment".to_string(),
                }),
                ),
                (
                    0,
                VNode::Label(VLabel {
                    text: state.to_string(),
                }),
                ),
            ]
        }
    }
}
