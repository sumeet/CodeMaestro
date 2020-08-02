use yew::html;
use yew::Callback;
use yew::Component;
use yew::ComponentLink;
use yew::Html;
use yew::InputData;
use yew::Properties;

use super::rgba;
use crate::colorscheme;
use stdweb::web::event::KeyPressEvent;

pub struct TextInput {
    props: TextInputProps,
}

#[derive(PartialEq, Properties, Clone)]
pub struct TextInputProps {
    oninput: Callback<InputData>,
    onkeypress: Callback<KeyPressEvent>,
    existing_value: String,
}

impl Component for TextInput {
    type Message = ();
    type Properties = TextInputProps;

    fn create(props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Self { props }
    }

    fn update(&mut self, _msg: Self::Message) -> bool {
        false
    }

    fn change(&mut self, props: Self::Properties) -> bool {
        let mut props = props;
        std::mem::swap(&mut self.props, &mut props);
        props != self.props
    }

    fn view(&self) -> Html {
        // just cloning some Rcs here
        let oninput = self.props.oninput.clone();
        let onkeypress = self.props.onkeypress.clone();
        html! {
            <input type="text",
               style=format!("display: block; background-color: {};", rgba(colorscheme!(input_bg_color))),
               autocomplete="off",
               value=self.props.existing_value,
               oninput=oninput,
               onkeypress=onkeypress, />
        }
    }
}
