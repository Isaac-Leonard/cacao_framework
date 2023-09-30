use cacao::layout::{Layout, LayoutConstraint};

/// Takes a list of views, a parent view that  contains them and returns layout constraints that will position them from top to bottom separated by the specified padding.
/// The padding is also applied to the sides of each view.
pub fn top_to_bottom(
    views: Vec<&dyn Layout>,
    parent: &impl Layout,
    padding: f32,
) -> Vec<LayoutConstraint> {
    let (top, bottom) = if let (Some(first), Some(last)) = (views.first(), views.last()) {
        (
            first
                .get_top()
                .constraint_equal_to(&parent.get_top())
                .offset(padding),
            last.get_bottom()
                .constraint_equal_to(&parent.get_bottom())
                .offset(padding),
        )
    } else {
        // No views were passed
        return Vec::new();
    };
    let adjoining_constraints = views
        .array_windows::<2>()
        .map(|[a, b]| a.get_bottom().constraint_equal_to(&b.get_top()));
    let side_constraints = views.iter().flat_map(|view| {
        [
            view.get_leading()
            .constraint_equal_to(&parent.get_leading())
                .offset(padding),
            view.get_trailing()
                .constraint_equal_to(&parent.get_trailing())
                .offset(padding),
        ]
    });
    vec![top, bottom]
        .into_iter()
        .chain(adjoining_constraints)
        .chain(side_constraints)
        .collect()
}
