mod component;
mod layout;
pub use component::*;

#[cfg(test)]
mod tests {
    /// Demonstrates the implementation of a simple counter component
    use super::*;
    pub struct CustomComponent;

    impl Renderable for CustomComponent {
        type State = u32;
        fn render(state: &Self::State) -> Vec<crate::component::Discripter<Self::State>> {
            vec![
                Discripter {
                    kind: ComponentType::Button(Some(|state| *state += 1)),
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
