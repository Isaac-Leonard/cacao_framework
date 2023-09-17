#[macro_export]
macro_rules! view{
    ($($name:ident {
		$($field:ident: $value:expr, )*
	}, )*) => {
		{
			use crate::macros::{Custom, Label};
			vec![$(
				if stringify!($name)=="Label"{
					Component::Label(Label {
						$($field: $value,)*
							..Label::default()
					})
				}else{
					Component::Custom(Custom {
						$( $field: $value,)*
							..Custom::new(stringify!($name).to_string())
					})
				},
			)*]
		}
    };
}

pub struct Label {
    pub text: String,
    pub colour: String,
}

impl Default for Label {
    fn default() -> Self {
        Self {
            text: "".to_owned(),
            colour: "white".to_owned(),
        }
    }
}

pub struct Custom {
    pub name: String,
    pub text: String,
    pub colour: String,
}
impl Custom {
    pub fn new(name: String) -> Self {
        Self {
            name,
            text: "".to_owned(),
            colour: "white".to_owned(),
        }
    }
}

pub enum Component {
    Label(Label),
    Custom(Custom),
}
