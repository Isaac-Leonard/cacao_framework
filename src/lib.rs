mod component;
mod layout;
pub use component::*;

#[cfg(test)]
mod tests {
    /// Demonstrates the implementation of a simple counter component
    use super::*;
    #[derive(PartialEq, Clone)]
    pub struct CustomComponent;

    impl Renderable for CustomComponent {
        type Props = ();
        type State = u32;
        fn render(
            _props: &Self::Props,
            state: &Self::State,
        ) -> Vec<crate::component::Discripter<Self>> {
            vec![
                Discripter {
                    kind: ComponentType::Button(Some(|_, state| *state += 1)),
                    text: "Increment".to_string(),
                },
                Discripter {
                    kind: ComponentType::Label,
                    text: state.to_string(),
                },
            ]
        }
    }
}
