You are working on an Elm SPA (Single Page Application) that implements the RealWorld "Conduit" spec — a Medium-like blogging platform.

## Project Structure

The project uses Elm 0.19.1 with a single `src/` source directory. The `elm.json` is at the project root.

### Key modules:

- **`Main.elm`** — Application entry point. Defines the top-level `Model` (a union type with one variant per page), `Msg`, `update`, `view`, and `subscriptions`. Routes are handled in `changeRouteTo`.
- **`Route.elm`** — URL routing. Defines `Route` type with variants: `Home`, `Root`, `Login`, `Logout`, `Register`, `Settings`, `Article Slug`, `Profile Username`, `NewArticle`, `EditArticle Slug`. Provides `fromUrl`, `href`, `replaceUrl`.
- **`Page.elm`** — Page chrome (header/navbar/footer). Defines `Page` type for navbar active state. `view` wraps page content with layout.
- **`Session.elm`** — Session state: `LoggedIn Nav.Key Viewer | Guest Nav.Key`. Provides `viewer`, `cred`, `navKey`.
- **`Api.elm`** — Port module for API communication. Defines opaque `Cred` type (credentials), HTTP helpers (`get`, `post`, `put`, `delete`), auth persistence via ports (`storeCredWith`, `logout`, `viewerChanges`), and error handling.
- **`Viewer.elm`** — The logged-in user. Wraps `Cred` with avatar/username.

### Page modules (`Page.*`):
Each page follows the same pattern: `Model`, `Msg`, `init`, `update`, `view`, `subscriptions`, `toSession`.
- `Page.Home` — Homepage with article feed tabs (`FeedTab`: `YourFeed Cred | GlobalFeed | TagFeed Tag`)
- `Page.Article` — Single article view
- `Page.Article.Editor` — Article editor (new and edit)
- `Page.Login`, `Page.Register` — Auth forms
- `Page.Settings` — User settings
- `Page.Profile` — User profile with article feed tabs (`MyArticles | FavoritedArticles`)
- `Page.Blank`, `Page.NotFound` — Placeholder pages

### Domain modules:
- `Article.elm` — Article type with `Full` and `Preview` variants
- `Article.Body` — Article body (markdown wrapper)
- `Article.Feed` — Reusable article feed with its own Model/Msg/update/view
- `Article.Slug`, `Article.Tag`, `Article.Comment` — Value types
- `Author.elm` — Author type: `IsFollowing FollowedAuthor | IsNotFollowing UnfollowedAuthor | IsViewer Cred Profile`
- `Profile.elm`, `Username.elm`, `Email.elm`, `Avatar.elm` — User-related types
- `Api.Endpoint` — API endpoint URLs

### Patterns:
- Pages are wired into `Main.elm` via `updateWith` helper that maps sub-messages
- The `Api` module uses ports for localStorage persistence
- Navigation uses fragment-based routing (`#/path`)
- `elm make src/Main.elm` compiles the entire application
