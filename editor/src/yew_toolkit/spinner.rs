use yew::html;
use yew::Component;
use yew::ComponentLink;
use yew::Html;

pub struct Spinner {}

impl Component for Spinner {
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
            <div class="spinner", >
                {" "}
            </div>
        }
    }
}
