Extract the `Cred` type and its directly related functions from `Api.elm` into a new `Api.Cred` module.

Specifically, move these to `src/Api/Cred.elm`:
- The `Cred` type definition
- The `username` function
- The `credHeader` function
- The `credDecoder` function

The new `Api.Cred` module should expose `Cred`, `username`, `credHeader`, and `credDecoder`.

Update `Api.elm` to import from `Api.Cred` instead of defining these locally. All other modules that import `Cred` from `Api` should continue to work — either update them to import from `Api.Cred` directly, or re-export `Cred` from `Api`.

Make sure the project compiles with `elm make src/Main.elm` after the extraction.
