port module Sample exposing (Model, Msg(..), update, view)

import Html exposing (Html, div, text)
import Html.Attributes as Attr


{-| The model for our app -}
type alias Model =
    { count : Int
    , name : String
    }


{-| Messages for the update function -}
type Msg
    = Increment
    | Decrement
    | Reset


update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment ->
            { model | count = model.count + 1 }

        Decrement ->
            { model | count = model.count - 1 }

        Reset ->
            { model | count = 0 }


view : Model -> Html Msg
view model =
    div []
        [ text (String.fromInt model.count) ]


helper x =
    x + 1


port sendMessage : String -> Cmd msg
