use yew::html;
use yew::Component;
use yew::ComponentLink;
use yew::Html;

pub struct Separator {}

impl Component for Separator {
    type Message = ();
    type Properties = ();

    fn create(_props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Self {}
    }

    fn update(&mut self, _msg: Self::Message) -> bool {
        false
    }

    fn change(&mut self, _props: Self::Properties) -> bool {
        false
    }

    fn view(&self) -> Html {
        html! {
            <hr />
        }
    }
}
