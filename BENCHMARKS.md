# elmq Benchmark Analysis

## Per-Scenario Detail

> Pricing: input $3.00/M · output $15.00/M · cache read $0.30/M · cache write $3.75/M (Sonnet 4.6)

| SCENARIO | ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 01-add-feature | control | 497 | 4,835 | 🟢 528,312 | 🟢 31,696 | 🟢 $0.3514 ±$0.0284 | 🟢 22 ±1 | 🟢 5:31 | ✅ 5/5 |
|  | treatment | 🟢 25 | 🟢 4,748 | 576,751 | 37,452 | $0.3848 ±$0.0609 | 25 ±4 | 5:48 | ✅ 5/5 |
| | | | | | | | | | |
| 02-rename-module | control | 93 | 2,199 | 224,110 | 16,736 | $0.1633 ±$0.0359 | 17 ±2 | 0:42 | ✅ 5/5 |
|  | treatment | 🟢 5 | 🟢 362 | 🟢 53,759 | 🟢 11,179 | 🟢 $0.0635 ±$0.0043 | 🟢 3 ±1 | 🟢 0:12 | ✅ 5/5 |
| | | | | | | | | | |
| 03-extract-module | control | 24 | 16,931 | 746,739 | 59,968 | $0.7029 ±$0.4502 ⚠️ | 43 ±28 ⚠️ | 4:42 | ❌ 4/5 |
|  | treatment | 🟢 18 | 🟢 5,717 | 🟢 395,236 | 🟢 22,621 | 🟢 $0.2892 ±$0.0367 | 🟢 17 ±3 | 🟢 1:59 | ✅ 5/5 |
| | | | | | | | | | |
| 04-add-route | control | 🟢 5 | 🟢 1,083 | 🟢 113,664 | 🟢 6,325 | 🟢 $0.0741 ±$0.1482 ⚠️ | 🟢 5 ±11 ⚠️ | 🟢 0:18 | ❌ 1/4 |
|  | treatment | 25 | 4,398 | 595,515 | 22,981 | $0.3309 ±$0.0383 | 25 ±2 | 1:39 | ✅ 5/5 |
| | | | | | | | | | |
| 05-add-variant | control | 🟢 9 | 🟢 2,063 | 🟢 130,307 | 🟢 13,817 | 🟢 $0.1219 | 🟢 7 | 🟢 0:36 | ✅ 1/1 |
|  | treatment | 13 | 2,560 | 220,460 | 15,279 | $0.1619 ±$0.0323 | 11 ±2 | 0:53 | ✅ 5/5 |

## Overall

| ARM | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---|
| control | $1.4135 | 94 | 11:51 | ❌ 16/20 |
| treatment | 🟢 $1.2302 | 🟢 81 | 🟢 10:33 | ✅ 25/25 |

- 🟢 **Δ cost** (trt − ctrl): $-0.1833 (-13.0%) ± $0.4841
- 🟢 **Δ turns** (trt − ctrl): -13 (-13.8%) ± 31

## Tool Breakdown

### control (2026-04-10T060715-control-5)

#### ✅ 01-add-feature

| Count | Tool |
|---:|---|
| 15 | Read |
| 15 | Edit |
| 2 | Write |
| 2 | Bash |
| 1 | ToolSearch |
| 1 | Grep |
| 1 | Glob |
| 1 | Agent |

#### ✅ 02-rename-module

| Count | Tool |
|---:|---|
| 6 | Edit |
| 5 | Read |
| 4 | Bash |
| 2 | Write |
| 2 | Glob |
| 1 | ToolSearch |
| 1 | Grep |

#### ✅ 03-extract-module

| Count | Tool |
|---:|---|
| 19 | Edit |
| 11 | Bash |
| 5 | Read |
| 3 | Grep |
| 2 | Write |
| 1 | ToolSearch |
| 1 | Glob |

#### ✅ 04-add-route

| Count | Tool |
|---:|---|
| 14 | Edit |
| 5 | Read |
| 2 | Write |
| 2 | Glob |
| 2 | Bash |
| 1 | ToolSearch |
| 1 | Grep |

#### ✅ 05-add-variant

| Count | Tool |
|---:|---|
| 5 | Edit |
| 2 | Read |
| 2 | Bash |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Grep |
| 1 | Glob |

### treatment (2026-04-10T060715-treatment-5)

#### ✅ 01-add-feature

| Count | Tool |
|---:|---|
| 24 | Bash |
| 2 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |

#### ✅ 02-rename-module

| Count | Tool |
|---:|---|
| 3 | Bash |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |

#### ✅ 03-extract-module

| Count | Tool |
|---:|---|
| 15 | Bash |
| 2 | Read |
| 2 | Edit |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Grep |
| 1 | Glob |

#### ✅ 04-add-route

| Count | Tool |
|---:|---|
| 28 | Bash |
| 2 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |

#### ✅ 05-add-variant

| Count | Tool |
|---:|---|
| 12 | Bash |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |

## Tool Call Details

### control (2026-04-10T060715-control-5)

#### ✅ 01-add-feature

1. `Agent` Explore codebase structure [Explore]
2. `Read` src/Route.elm
3. `Read` src/Main.elm
4. `Read` src/Page.elm
5. `Read` src/Page/Settings.elm
6. `Read` src/Route.elm
7. `Read` src/Page.elm
8. `Read` src/Page/Settings.elm
9. `Read` src/Main.elm
10. `Read` src/Page/Blank.elm
11. `Read` src/Session.elm
12. `Write` src/Page/Bookmarks.elm
13. `Edit` src/Route.elm
14. `Edit` src/Route.elm
15. `Edit` src/Route.elm
16. `Edit` src/Page.elm
17. `Edit` src/Page.elm
18. `Edit` src/Page.elm
19. `Edit` src/Main.elm
20. `Edit` src/Main.elm
21. `Edit` src/Main.elm
22. `Edit` src/Main.elm
23. `Edit` src/Main.elm
24. `Edit` src/Main.elm
25. `Edit` src/Main.elm
26. `Edit` src/Main.elm
27. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 02-rename-module

1. `Bash` grep -rn "Article.Body" src/ --include="*.elm" 
2. `Glob` src/Article/Body.elm
3. `Read` src/Article/Body.elm
4. `Write` src/Article/Content.elm
5. `Read` src/Article.elm
6. `Read` src/Page/Article.elm
7. `Read` src/Page/Article/Editor.elm
8. `Edit` src/Article.elm
9. `Edit` src/Page/Article.elm
10. `Edit` src/Page/Article/Editor.elm
11. `Edit` src/Page/Article/Editor.elm
12. `Edit` src/Page/Article.elm
13. `Bash` rm src/Article/Body.elm 
14. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 03-extract-module

1. `Read` src/Api.elm
2. `Grep` import Api in src
3. `Bash` ls src/Api/ 
4. `Write` src/Api/Cred.elm
5. `Edit` src/Api.elm
6. `Read` src/Api.elm
7. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 
8. `Grep` Api\.username|username.*Cred|Cred.*username in src
9. `Bash` for f in src/Main.elm src/Session.elm src/Page/Home.elm src/Page.elm src/Article/Comment.elm src/Pag...
10. `Bash` grep -n "Api\." src/Main.elm | grep -v "Api\.Endpoint\|import Api" 
11. `Bash` for f in src/Main.elm src/Session.elm src/Page/Home.elm src/Page.elm src/Article/Comment.elm src/Pag...
12. `Edit` src/Api.elm
13. `Edit` src/Api.elm
14. `Read` src/Page.elm
15. `Read` src/Profile.elm
16. `Edit` src/Page.elm
17. `Edit` src/Profile.elm
18. `Edit` src/Main.elm
19. `Edit` src/Session.elm
20. `Edit` src/Page/Home.elm
21. `Edit` src/Article/Comment.elm
22. `Edit` src/Page/Article/Editor.elm
23. `Edit` src/Page/Article.elm
24. `Edit` src/Page/Settings.elm
25. `Edit` src/Page/Register.elm
26. `Edit` src/Page/Login.elm
27. `Edit` src/Article.elm
28. `Edit` src/Article/Feed.elm
29. `Edit` src/Article/Tag.elm
30. `Edit` src/Page/Profile.elm
31. `Bash` head -5 src/Main.elm echo "---" head -5 src/Session.elm echo "---" head -10 src/Page/Home.elm echo "...
32. `Bash` head -10 src/Page/Article.elm echo "---" head -6 src/Page/Settings.elm echo "---" head -5 src/Page/R...
33. `Bash` head -5 src/Article/Feed.elm echo "---" head -6 src/Article/Tag.elm echo "---" head -9 src/Page/Prof...
34. `Bash` cd /bench/results/control/2026-04-10T060715-control-5/workdir # Files that use Api.xxx qualified acc...
35. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

#### ✅ 04-add-route

1. `Read` src/Route.elm
2. `Read` src/Page.elm
3. `Read` src/Main.elm
4. `Glob` src/Page/*.elm
5. `Read` src/Page/Bookmarks.elm
6. `Edit` src/Route.elm
7. `Edit` src/Route.elm
8. `Edit` src/Page.elm
9. `Edit` src/Page.elm
10. `Edit` src/Page.elm
11. `Write` src/Page/Drafts.elm
12. `Edit` src/Main.elm
13. `Edit` src/Main.elm
14. `Edit` src/Main.elm
15. `Edit` src/Main.elm
16. `Edit` src/Main.elm
17. `Edit` src/Main.elm
18. `Edit` src/Main.elm
19. `Edit` src/Main.elm
20. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

#### ✅ 05-add-variant

1. `Read` src/Page/Home.elm
2. `Edit` src/Page/Home.elm
3. `Edit` src/Page/Home.elm
4. `Edit` src/Page/Home.elm
5. `Edit` src/Page/Home.elm
6. `Bash` elm make src/Main.elm 2>&1 

### treatment (2026-04-10T060715-treatment-5)

#### ✅ 01-add-feature

1. `Bash` elmq list src/Route.elm src/Page.elm src/Main.elm 
2. `Bash` ls src/Page/ 
3. `Bash` elmq get src/Route.elm Route parser routeToPieces 
4. `Bash` elmq get src/Page.elm viewMenu isActive 
5. `Bash` elmq get src/Main.elm Model Msg toSession changeRouteTo update subscriptions view 
6. `Bash` cat src/Page/Settings.elm 
7. `Bash` elmq get src/Page.elm Page 
8. `Bash` cat src/Page/Blank.elm 
9. `Write` src/Page/Bookmarks.elm
10. `Bash` elmq patch --old 'type Route = Home | Root | Login | Logout | Register | Settings | Article Slug | P...
11. `Bash` elmq patch --old ' , Parser.map EditArticle (s "editor" </> Slug.urlParser) ]' --new ' , Parser.map ...
12. `Bash` elmq patch --old ' EditArticle slug -> [ "editor", Slug.toString slug ]' --new ' EditArticle slug ->...
13. `Bash` elmq patch --old 'type Page = Other | Home | Login | Register | Settings | Profile Username | NewArt...
14. `Bash` elmq patch --old ' [ linkTo Route.NewArticle [ i [ class "ion-compose" ] [], text "\u{00A0}New Post"...
15. `Bash` elmq patch --old ' ( NewArticle, Route.NewArticle ) -> True _ -> False' --new ' ( NewArticle, Route....
16. `Bash` elmq import add src/Main.elm 'Page.Bookmarks as Bookmarks' 
17. `Bash` elmq patch --old ' | Editor (Maybe Slug) Editor.Model' --new ' | Editor (Maybe Slug) Editor.Model | ...
18. `Bash` elmq patch --old ' | GotEditorMsg Editor.Msg' --new ' | GotEditorMsg Editor.Msg | GotBookmarksMsg Bo...
19. `Bash` elmq patch --old ' Editor _ editor -> Editor.toSession editor' --new ' Editor _ editor -> Editor.toS...
20. `Bash` elmq patch --old ' Just (Route.Article slug) -> Article.init session slug |> updateWith Article GotA...
21. `Bash` elmq patch --old ' ( GotEditorMsg subMsg, Editor slug editor ) -> Editor.update subMsg editor |> upd...
22. `Bash` elmq patch --old ' Editor _ editor -> Sub.map GotEditorMsg (Editor.subscriptions editor)' --new ' Ed...
23. `Bash` elmq patch --old ' Editor Nothing editor -> viewPage Page.NewArticle GotEditorMsg (Editor.view edito...
24. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 02-rename-module

1. `Bash` elmq mv src/Article/Body.elm src/Article/Content.elm 
2. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 03-extract-module

1. `Bash` elmq list src/Api.elm 
2. `Bash` elmq get src/Api.elm Cred username credHeader credDecoder 
3. `Bash` elmq refs src/Api.elm Cred username credHeader credDecoder 
4. `Bash` elmq move-decl src/Api.elm --to src/Api/Cred.elm Cred username credHeader credDecoder --dry-run 2>&1...
5. `Bash` elmq move-decl src/Api.elm --to src/Api/Cred.elm Cred username credHeader credDecoder 2>&1 
6. `Bash` elm make src/Main.elm 2>&1 
7. `Bash` elmq get src/Api.elm storeCredWith 
8. `Bash` elmq list src/Api.elm | head -20 
9. `Bash` elmq list src/Api/Cred.elm 
10. `Bash` elmq expose src/Api/Cred.elm 'Cred(..)' 2>&1 && elmq unexpose src/Api/Cred.elm Cred 2>&1 
11. `Bash` elmq import add src/Api.elm 'Api.Cred exposing (Cred(..), credDecoder, credHeader, username)' 2>&1 
12. `Bash` elm make src/Main.elm 2>&1 
13. `Bash` elmq list src/Api/Cred.elm 
14. `Read` src/Api/Cred.elm
15. `Edit` src/Api/Cred.elm
16. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 04-add-route

1. `Bash` elmq list src/Route.elm src/Page.elm src/Main.elm 
2. `Bash` elmq get src/Route.elm Route parser routeToString routeToPieces 
3. `Bash` elmq get src/Page.elm Page viewMenu isActive 
4. `Bash` elmq get src/Main.elm Model Msg toSession changeRouteTo update subscriptions view 
5. `Bash` cat src/Page/Bookmarks.elm 
6. `Write` src/Page/Drafts.elm
7. `Bash` elmq variant add src/Route.elm --type Route 'Drafts' 
8. `Bash` elmq patch --old 'Bookmarks -> [ "bookmarks" ]' --new 'Bookmarks -> [ "bookmarks" ] Drafts -> [ "dra...
9. `Bash` elmq patch --old ', Parser.map Bookmarks (s "bookmarks") ]' --new ', Parser.map Bookmarks (s "bookma...
10. `Bash` elmq variant add src/Page.elm --type Page 'Drafts' 
11. `Bash` elmq patch --old ' ( Bookmarks, Route.Bookmarks ) -> True _ ->' --new ' ( Bookmarks, Route.Bookmarks...
12. `Bash` elmq patch --old ' [ linkTo Route.NewArticle [ i [ class "ion-compose" ] [], text "\u{00A0}New Post"...
13. `Bash` elmq variant add src/Main.elm --type Model 'Drafts Drafts.Model' 
14. `Bash` elmq variant add src/Main.elm --type Msg 'GotDraftsMsg Drafts.Msg' 
15. `Bash` elmq get src/Main.elm toSession subscriptions update view changeRouteTo 
16. `Bash` elmq patch --old ' Drafts _ -> Debug.todo "Drafts" ## subscriptions' --new ' Drafts drafts -> Drafts...
17. `Bash` elmq patch --old ' Drafts _ -> Debug.todo "Drafts" ## update' --new ' Drafts drafts -> Sub.map GotDr...
18. `Bash` elmq get src/Main.elm toSession subscriptions 
19. `Bash` elmq patch --old ' Drafts _ -> Debug.todo "Drafts"' --new ' Drafts drafts -> Drafts.toSession drafts...
20. `Bash` elmq patch --old ' Drafts _ -> Debug.todo "Drafts"' --new ' Drafts drafts -> viewPage Page.Drafts Go...
21. `Bash` elmq patch --old ' ( _, Drafts _ ) -> Debug.todo "Drafts" ( GotDraftsMsg _, _ ) -> Debug.todo "GotDr...
22. `Bash` elmq patch --old ' Just Route.Bookmarks -> Bookmarks.init session |> updateWith Bookmarks GotBookmar...
23. `Bash` elmq import add src/Main.elm 'Page.Drafts as Drafts' 
24. `Bash` elm make src/Main.elm 2>&1 
25. `Bash` elmq patch --old ' Drafts -> [ "drafts" ] Drafts -> Debug.todo "Drafts"' --new ' Drafts -> [ "drafts...
26. `Bash` elm make src/Main.elm 2>&1 
27. `Bash` elmq patch --old ' ( GotSession session, Redirect _ ) -> ( Redirect session , Route.replaceUrl (Sess...
28. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 05-add-variant

1. `Bash` elmq list src/Page/Home.elm 
2. `Bash` elmq variant cases src/Page/Home.elm --type FeedTab 
3. `Bash` elmq get src/Page/Home.elm FeedTab viewTabs yourFeed globalFeed tagFeed 
4. `Bash` elmq variant add src/Page/Home.elm --type FeedTab 'BookmarkedFeed Cred' \ --fill 'viewTabs=Bookmarke...
5. `Bash` elmq set src/Page/Home.elm << 'ELM' bookmarkedFeed : Cred -> ( String, Msg ) bookmarkedFeed cred = (...
6. `Bash` elmq get src/Page/Home.elm viewTabs 
7. `Bash` elmq patch --old ' YourFeed cred -> Feed.viewTabs [] (yourFeed cred) [ globalFeed ]' \ --new ' YourF...
8. `Bash` elmq patch --old ' GlobalFeed -> let otherTabs = case maybeCred of Just cred -> [ yourFeed cred ] No...
9. `Bash` elmq patch --old ' TagFeed tag -> let otherTabs = case maybeCred of Just cred -> [ yourFeed cred, gl...
10. `Bash` elmq patch --old ' BookmarkedFeed cred -> Feed.viewTabs [ yourFeed cred, globalFeed ] (bookmarkedFee...
11. `Bash` elm make src/Main.elm 2>&1 

