Add a new "Bookmarks" page to this Elm SPA. Users who are logged in should be able to view their bookmarked articles.

Requirements:
- Create a new `Page.Bookmarks` module following the same pattern as the other page modules (Model, Msg, init, update, view, subscriptions, toSession)
- Add a `Bookmarks` variant to the `Route` type in `Route.elm`, mapped to the URL path "bookmarks"
- Wire the route into `Main.elm` — add the `Bookmarks` model variant, message variant, and handle it in `changeRouteTo`, `update`, `view`, `subscriptions`, and `toSession`
- Add a "Bookmarks" link to the navbar in `Page.elm` for logged-in users (next to "New Post"), with the icon class `ion-bookmark`
- The page should display a simple placeholder message like "Your bookmarked articles will appear here." with appropriate styling using the existing CSS classes
- The page requires authentication — redirect to the login page if the user is not logged in

Make sure the project compiles with `elm make src/Main.elm`.
