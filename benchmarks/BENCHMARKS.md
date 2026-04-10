# elmq Benchmark Analysis

## Per-Scenario Detail

> Pricing: input $3.00/M · output $15.00/M · cache read $0.30/M · cache write $3.75/M (Sonnet 4.6)

### 01-add-feature

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 358 | 4,908 | 514,369 | 45,469 | $0.3995 ±$0.0248 | 22 ±1 | 🟢 **1:53** | ✅ 5/5 |
| treatment | 🟢 **19** | 🟢 **4,822** | 🟢 **361,442** | 🟢 **42,544** | 🟢 **$0.3404 ±$0.0526** | 🟢 **16 ±4** | 1:57 | ✅ 5/5 |

### 02-rename-module

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 28 | 2,243 | 211,820 | 14,582 | $0.1520 ±$0.0335 | 17 ±3 | 0:48 | ✅ 5/5 |
| treatment | 🟢 **5** | 🟢 **328** | 🟢 **49,987** | 🟢 **11,058** | 🟢 **$0.0614 ±$0.0035** | 🟢 **3 ±0** | 🟢 **0:11** | ✅ 5/5 |

### 03-extract-module

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 21 | 16,940 | 616,985 | 60,189 | $0.6650 ±$0.2594 | 35 ±21 ⚠️ | 4:31 | ✅ 5/5 |
| treatment | 🟢 **7** | 🟢 **1,512** | 🟢 **98,659** | 🟢 **15,952** | 🟢 **$0.1121 ±$0.0096** | 🟢 **6 ±0** | 🟢 **0:32** | ✅ 5/5 |

### 04-add-route

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 20 | 4,499 | 466,403 | 22,122 | $0.2904 ±$0.0221 | 22 ±1 | 1:59 | ✅ 5/5 |
| treatment | 🟢 **15** | 🟢 **3,473** | 🟢 **300,412** | 🟢 **20,503** | 🟢 **$0.2192 ±$0.0222** | 🟢 **14 ±1** | 🟢 **1:11** | ✅ 5/5 |

### 05-add-variant

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 🟢 **8** | 🟢 **2,524** | 🟢 **127,149** | 🟢 **14,376** | 🟢 **$0.1299 ±$0.0063** | 🟢 **6 ±0** | 🟢 **0:45** | ✅ 5/5 |
| treatment | 11 | 2,568 | 194,670 | 16,066 | $0.1572 ±$0.0139 | 9 ±1 | 0:51 | ✅ 5/5 |


## Overall

| ARM | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---|
| control | $1.6368 | 102 | 9:57 | ✅ 25/25 |
| treatment | 🟢 **$0.8902** | 🟢 **48** | 🟢 **4:43** | ✅ 25/25 |

- 🟢 **Δ cost** (trt − ctrl): $-0.7466 (-45.6%) ± $0.2704
- 🟢 **Δ turns** (trt − ctrl): -54 (-52.9%) ± 22

## Tool Breakdown

### control (2026-04-10T073616-control-4)

#### ✅ 01-add-feature

| Count | Tool |
|---:|---|
| 15 | Edit |
| 5 | Read |
| 2 | Write |
| 2 | Bash |
| 1 | ToolSearch |
| 1 | Grep |
| 1 | Glob |

#### ✅ 02-rename-module

| Count | Tool |
|---:|---|
| 8 | Read |
| 8 | Edit |
| 4 | Bash |
| 3 | Grep |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Glob |

#### ✅ 03-extract-module

| Count | Tool |
|---:|---|
| 12 | Edit |
| 7 | Read |
| 6 | Bash |
| 3 | Grep |
| 2 | Write |
| 1 | ToolSearch |
| 1 | Glob |

#### ✅ 04-add-route

| Count | Tool |
|---:|---|
| 15 | Edit |
| 5 | Read |
| 2 | Write |
| 2 | Bash |
| 1 | ToolSearch |
| 1 | Grep |
| 1 | Glob |

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

### treatment (2026-04-10T073616-treatment-4)

#### ✅ 01-add-feature

| Count | Tool |
|---:|---|
| 32 | Bash |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |
| 1 | Agent |

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
| 6 | Bash |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |

#### ✅ 04-add-route

| Count | Tool |
|---:|---|
| 13 | Bash |
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

### control (2026-04-10T073616-control-4)

#### ✅ 01-add-feature

1. `Read` src/Route.elm
2. `Read` src/Page.elm
3. `Read` src/Main.elm
4. `Read` src/Page/Settings.elm
5. `Write` src/Page/Bookmarks.elm
6. `Edit` src/Route.elm
7. `Edit` src/Route.elm
8. `Edit` src/Route.elm
9. `Edit` src/Page.elm
10. `Edit` src/Page.elm
11. `Edit` src/Page.elm
12. `Edit` src/Main.elm
13. `Edit` src/Main.elm
14. `Edit` src/Main.elm
15. `Edit` src/Main.elm
16. `Edit` src/Main.elm
17. `Edit` src/Main.elm
18. `Edit` src/Main.elm
19. `Edit` src/Main.elm
20. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

#### ✅ 02-rename-module

1. `Grep` Article\.Body
2. `Read` src/Article/Body.elm
3. `Grep` Article\.Body
4. `Read` src/Article.elm
5. `Read` src/Page/Article.elm
6. `Read` src/Page/Article/Editor.elm
7. `Bash` cp src/Article/Body.elm src/Article/Content.elm 
8. `Edit` src/Article/Content.elm
9. `Read` src/Article/Content.elm
10. `Edit` src/Article/Content.elm
11. `Edit` src/Article.elm
12. `Edit` src/Page/Article.elm
13. `Read` src/Page/Article.elm
14. `Edit` src/Page/Article.elm
15. `Edit` src/Page/Article/Editor.elm
16. `Read` src/Page/Article/Editor.elm
17. `Edit` src/Page/Article/Editor.elm
18. `Bash` rm src/Article/Body.elm 
19. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 03-extract-module

1. `Read` src/Api.elm
2. `Grep` import Api in src
3. `Write` src/Api/Cred.elm
4. `Edit` src/Api.elm
5. `Read` src/Api.elm
6. `Edit` src/Api/Cred.elm
7. `Edit` src/Api/Cred.elm
8. `Edit` src/Api.elm
9. `Edit` src/Api.elm
10. `Edit` src/Api.elm
11. `Edit` src/Api.elm
12. `Read` src/Api/Cred.elm
13. `Read` src/Api.elm
14. `Bash` elm make src/Main.elm 2>&1 
15. `Grep` Api\. in src
16. `Bash` for f in src/Page.elm src/Page/Home.elm src/Author.elm src/Session.elm src/Article.elm src/Article/T...
17. `Bash` for f in src/Page.elm src/Page/Home.elm src/Author.elm src/Session.elm src/Article.elm src/Article/T...
18. `Edit` src/Api.elm
19. `Read` src/Page.elm
20. `Read` src/Profile.elm
21. `Edit` src/Page.elm
22. `Edit` src/Profile.elm
23. `Edit` src/Page/Home.elm
24. `Bash` for f in src/Page/Home.elm src/Author.elm src/Session.elm src/Article.elm src/Article/Tag.elm src/Pa...
25. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 04-add-route

1. `Read` src/Route.elm
2. `Read` src/Page.elm
3. `Read` src/Main.elm
4. `Read` src/Page/Bookmarks.elm
5. `Write` src/Page/Drafts.elm
6. `Edit` src/Route.elm
7. `Edit` src/Page.elm
8. `Edit` src/Route.elm
9. `Edit` src/Route.elm
10. `Edit` src/Page.elm
11. `Edit` src/Page.elm
12. `Edit` src/Main.elm
13. `Edit` src/Main.elm
14. `Edit` src/Main.elm
15. `Edit` src/Main.elm
16. `Edit` src/Main.elm
17. `Edit` src/Main.elm
18. `Edit` src/Main.elm
19. `Edit` src/Main.elm
20. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 05-add-variant

1. `Read` src/Page/Home.elm
2. `Edit` src/Page/Home.elm
3. `Edit` src/Page/Home.elm
4. `Edit` src/Page/Home.elm
5. `Edit` src/Page/Home.elm
6. `Bash` elm make src/Main.elm 2>&1 

### treatment (2026-04-10T073616-treatment-4)

#### ✅ 01-add-feature

1. `Agent` Explore existing page modules and routing [Explore]
2. `Bash` elmq list src/Route.elm 
3. `Bash` elmq list src/Page.elm 
4. `Bash` elmq list src/Main.elm 
5. `Bash` elmq get src/Route.elm Route parser href replaceUrl 
6. `Bash` elmq get src/Page.elm view 
7. `Bash` ls -la src/Page/ 
8. `Bash` elmq list src/Page/Settings.elm 
9. `Bash` elmq get src/Main.elm changeRouteTo update view subscriptions 
10. `Bash` elmq get src/Page.elm "Page" viewMenu isActive 
11. `Bash` elmq get src/Route.elm Route parser href replaceUrl && echo "---" && elmq get src/Page.elm Page view...
12. `Bash` elmq get src/Page.elm viewMenu viewHeader && echo "---" && elmq get src/Page/Settings.elm init toSes...
13. `Bash` elmq get src/Page/Article/Editor.elm initNew && echo "---" && elmq list src/Main.elm | head -30 
14. `Bash` elmq get src/Route.elm routeToString 
15. `Bash` elmq get src/Route.elm routeToPieces 
16. `Bash` cat > src/Page/Bookmarks.elm << 'ELM' module Page.Bookmarks exposing (Model, Msg, init, subscription...
17. `Bash` \ elmq patch --old ' | EditArticle Slug' --new ' | EditArticle Slug | Bookmarks' src/Route.elm Route...
18. `Bash` \ elmq patch --old ' | NewArticle' --new ' | NewArticle | Bookmarks' src/Page.elm Page && \ elmq pat...
19. `Bash` \ elmq import add src/Main.elm 'Page.Bookmarks as Bookmarks' && \ elmq patch --old ' | Editor (Maybe...
20. `Bash` elmq patch \ --old ' Just (Route.Article slug) -> Article.init session slug |> updateWith Article Go...
21. `Bash` elmq patch \ --old ' ( GotEditorMsg subMsg, Editor slug editor ) -> Editor.update subMsg editor |> u...
22. `Bash` elm make src/Main.elm --output /dev/null 2>&1 
23. `Bash` elmq patch \ --old ' Editor _ editor -> Editor.toSession editor' \ --new ' Editor _ editor -> Editor...

#### ✅ 02-rename-module

1. `Bash` elmq mv src/Article/Body.elm src/Article/Content.elm 
2. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

#### ✅ 03-extract-module

1. `Bash` elmq list src/Api.elm 
2. `Bash` elmq refs src/Api.elm Cred username credHeader credDecoder 
3. `Bash` elmq get src/Api.elm Cred username credHeader credDecoder 
4. `Bash` elmq move-decl src/Api.elm --to src/Api/Cred.elm Cred username credHeader credDecoder 
5. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 04-add-route

1. `Bash` elmq list src/Route.elm src/Page.elm src/Main.elm 
2. `Bash` elmq get src/Route.elm Route parser routeToPieces && elmq get src/Page.elm Page viewMenu isActive 
3. `Bash` elmq get src/Main.elm Model Msg toSession changeRouteTo update subscriptions 
4. `Bash` cat src/Page/Bookmarks.elm 
5. `Write` src/Page/Drafts.elm
6. `Bash` \ elmq patch --old ' | Bookmarks' --new ' | Bookmarks | Drafts' src/Route.elm Route && \ elmq patch ...
7. `Bash` \ elmq patch --old ' | Bookmarks' --new ' | Bookmarks | Drafts' src/Page.elm Page && \ elmq patch --...
8. `Bash` \ elmq patch --old ' | Bookmarks Bookmarks.Model' --new ' | Bookmarks Bookmarks.Model | Drafts Draft...
9. `Bash` \ elmq patch \ --old ' Just Route.Bookmarks -> case Session.cred session of Just _ -> Bookmarks.init...
10. `Bash` \ elmq patch \ --old ' ( GotBookmarksMsg subMsg, Bookmarks bookmarks ) -> Bookmarks.update subMsg bo...
11. `Bash` elmq get src/Main.elm view 
12. `Bash` elmq patch \ --old ' Bookmarks bookmarks -> viewPage Page.Bookmarks GotBookmarksMsg (Bookmarks.view ...
13. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

#### ✅ 05-add-variant

1. `Bash` elmq list src/Page/Home.elm 
2. `Bash` elmq get src/Page/Home.elm FeedTab viewTabs yourFeed globalFeed tagFeed fetchFeed 
3. `Bash` elmq variant cases src/Page/Home.elm --type FeedTab 
4. `Bash` elmq get src/Page/Home.elm view init 
5. `Bash` elmq variant add src/Page/Home.elm --type FeedTab 'BookmarkedFeed Cred' \ --fill 'viewTabs=Bookmarke...
6. `Bash` elmq get src/Page/Home.elm viewTabs bookmarkedFeed 2>/dev/null || elmq get src/Page/Home.elm viewTab...
7. `Bash` elmq patch --old ' BookmarkedFeed cred -> let otherTabs = case maybeCred of Just c -> [ yourFeed c, ...
8. `Bash` elmq set src/Page/Home.elm << 'ELM' bookmarkedFeed : Cred -> ( String, Msg ) bookmarkedFeed cred = (...
9. `Bash` elmq patch --old ' YourFeed cred -> Feed.viewTabs [] (yourFeed cred) [ globalFeed ]' \ --new ' YourF...
10. `Bash` elmq get src/Page/Home.elm viewTabs 
11. `Bash` elm make src/Main.elm 2>&1 

