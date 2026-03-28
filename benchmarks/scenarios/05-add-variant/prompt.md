Add a new `BookmarkedFeed Cred` variant to the `FeedTab` type in `src/Page/Home.elm`.

This represents a feed showing the user's bookmarked articles. It requires `Cred` because only authenticated users can have bookmarks.

Requirements:
- Add `BookmarkedFeed Cred` as a new variant of the `FeedTab` type
- Handle the new variant in every `case` expression that matches on `FeedTab` within `Page/Home.elm`
- For the feed fetching logic, the `BookmarkedFeed` tab should fetch from the same endpoint as `GlobalFeed` for now (this is a placeholder — the real endpoint would be added later)
- For the tab display, render it as a tab labeled "Bookmarked" with the same styling as the other feed tabs
- The `BookmarkedFeed` tab should only appear when the user is logged in

Make sure the project compiles with `elm make src/Main.elm`.
