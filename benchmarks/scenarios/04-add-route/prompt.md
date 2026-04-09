Add a "Drafts" feature to the application for viewing unpublished article drafts.

Requirements:
- Add a `Drafts` variant to the `Route` type in `Route.elm`, mapped to the URL path "drafts"
- Create a new `Page.Drafts` module following the existing page module pattern (Model, Msg, init, update, view, subscriptions, toSession)
- The page should show a heading "My Drafts" and a placeholder message "You have no drafts yet."
- Wire it into `Main.elm` — model variant, message variant, routing in `changeRouteTo`, and all the case expressions in `update`, `view`, `subscriptions`, and `toSession`
- Add a `Drafts` variant to the `Page` type in `Page.elm` for navbar active state tracking
- Add a "Drafts" link in the navbar for logged-in users, using the icon class `ion-document`
- The page requires authentication — redirect to login if not logged in

Make sure the project compiles with `elm make src/Main.elm`.
