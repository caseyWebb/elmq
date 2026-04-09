Rename the `Article.Body` module to `Article.Content` throughout the entire project.

This means:
- Rename the file `src/Article/Body.elm` to `src/Article/Content.elm`
- Update the module declaration inside the file from `module Article.Body` to `module Article.Content`
- Update every import statement across the project that references `Article.Body` to reference `Article.Content` instead
- Update any qualified references like `Article.Body.decoder` to `Article.Content.decoder`

Make sure the project compiles with `elm make src/Main.elm` after the rename.
