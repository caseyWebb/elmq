# elmq Benchmark Analysis

## Per-Scenario Detail

> Pricing: input $3.00/M · output $15.00/M · cache read $0.30/M · cache write $3.75/M (Sonnet 4.6)

### 01-add-feature

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 1,006 | 🟢 **5,221** | 574,437 | 41,214 | $0.4082 ±$0.0593 | 24 ±4 | 🟢 **2:11** | ✅ 5/5 |
| treatment | 🟢 **16** | 6,886 | 🟢 **325,358** | 🟢 **38,710** | 🟢 **$0.3461 ±$0.1081** | 🟢 **13 ±5** | 2:26 | ✅ 5/5 |

### 02-rename-module

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 93 | 2,188 | 211,709 | 13,579 | $0.1475 ±$0.0104 | 16 ±2 | 0:40 | ✅ 5/5 |
| treatment | 🟢 **5** | 🟢 **312** | 🟢 **46,201** | 🟢 **11,000** | 🟢 **$0.0598 ±$0.0003** | 🟢 **3 ±0** | 🟢 **0:10** | ✅ 5/5 |

### 03-extract-module

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 37 | 22,097 | 1,403,276 | 69,366 | $1.0127 ±$0.2579 | 53 ±12 | 5:54 | ✅ 5/5 |
| treatment | 🟢 **13** | 🟢 **5,192** | 🟢 **324,240** | 🟢 **26,669** | 🟢 **$0.2752 ±$0.3750 ⚠️** | 🟢 **16 ±23 ⚠️** | 🟢 **1:34** | ✅ 5/5 |

### 04-add-route

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 21 | 4,513 | 489,303 | 21,344 | $0.2946 ±$0.0081 | 21 ±1 | 1:28 | ✅ 5/5 |
| treatment | 🟢 **17** | 🟢 **3,547** | 🟢 **355,312** | 🟢 **20,720** | 🟢 **$0.2376 ±$0.0308** | 🟢 **15 ±3** | 🟢 **1:08** | ✅ 5/5 |

### 05-add-variant

| ARM | INPUT | OUTPUT | CACHE_R | CACHE_C | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| control | 🟢 **8** | 🟢 **2,576** | 🟢 **127,433** | 🟢 **14,418** | 🟢 **$0.1310 ±$0.0099** | 🟢 **6 ±0** | 🟢 **0:43** | ✅ 5/5 |
| treatment | 12 | 2,787 | 215,696 | 16,950 | $0.1701 ±$0.0312 | 10 ±2 | 0:55 | ✅ 5/5 |


## Overall

| ARM | COST | TURNS | TIME | PASS |
|---|---:|---:|---:|---|
| control | $1.9940 | 120 | 10:58 | ✅ 25/25 |
| treatment | 🟢 **$1.0888** | 🟢 **57** | 🟢 **6:15** | ✅ 25/25 |

- 🟢 **Δ cost** (trt − ctrl): $-0.9052 (-45.4%) ± $0.4739
- 🟢 **Δ turns** (trt − ctrl): -63 (-52.5%) ± 27

## Tool Breakdown

### control (2026-04-10T081651-control-5)

#### ✅ 01-add-feature

| Count | Tool |
|---:|---|
| 18 | Read |
| 15 | Edit |
| 6 | Grep |
| 2 | Write |
| 2 | Bash |
| 1 | ToolSearch |
| 1 | Glob |
| 1 | Agent |

#### ✅ 02-rename-module

| Count | Tool |
|---:|---|
| 7 | Edit |
| 5 | Read |
| 5 | Bash |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Grep |
| 1 | Glob |

#### ✅ 03-extract-module

| Count | Tool |
|---:|---|
| 67 | Grep |
| 57 | Read |
| 30 | Edit |
| 5 | Bash |
| 2 | Write |
| 1 | ToolSearch |
| 1 | Glob |
| 1 | Agent |

#### ✅ 04-add-route

| Count | Tool |
|---:|---|
| 15 | Edit |
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

### treatment (2026-04-10T081651-treatment-5)

#### ✅ 01-add-feature

| Count | Tool |
|---:|---|
| 12 | Bash |
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
| 5 | Bash |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |

#### ✅ 04-add-route

| Count | Tool |
|---:|---|
| 19 | Bash |
| 2 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |

#### ✅ 05-add-variant

| Count | Tool |
|---:|---|
| 10 | Bash |
| 1 | Write |
| 1 | ToolSearch |
| 1 | Read |
| 1 | Grep |
| 1 | Glob |
| 1 | Edit |

## Tool Call Details

### control (2026-04-10T081651-control-5)

#### ✅ 01-add-feature

1. `Agent` Explore existing page patterns [Explore]
2. `Read` src/Route.elm
3. `Read` src/Main.elm
4. `Read` src/Page.elm
5. `Read` src/Page/Blank.elm
6. `Read` src/Route.elm
7. `Read` src/Page.elm
8. `Read` src/Main.elm
9. `Read` src/Page/Blank.elm
10. `Read` src/Page/Settings.elm
11. `Grep` Guest|navKey|Route.Login|replaceUrl in src/Page/Settings.elm
12. `Grep` Session.cred|Session.viewer|isGuest|LoggedIn|Guest in src/Page/Settings.elm
13. `Grep` init in src/Page/Settings.elm
14. `Read` src/Page/Settings.elm
15. `Grep` Route.Login|replaceUrl.*Login|cred session in src/Page/Article/Editor.elm
16. `Grep` Route.Login|replaceUrl.*Login in src
17. `Read` src/Page/Article/Editor.elm
18. `Read` src/Page/Article/Editor.elm
19. `Read` src/Session.elm
20. `Write` src/Page/Bookmarks.elm
21. `Edit` src/Route.elm
22. `Edit` src/Route.elm
23. `Edit` src/Route.elm
24. `Edit` src/Main.elm
25. `Edit` src/Main.elm
26. `Edit` src/Main.elm
27. `Edit` src/Main.elm
28. `Edit` src/Main.elm
29. `Edit` src/Main.elm
30. `Edit` src/Main.elm
31. `Edit` src/Main.elm
32. `Edit` src/Page.elm
33. `Edit` src/Page.elm
34. `Edit` src/Page.elm
35. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 02-rename-module

1. `Bash` grep -rn "Article\.Body" src/ --include="*.elm" 
2. `Bash` cp src/Article/Body.elm src/Article/Content.elm 
3. `Read` src/Article/Content.elm
4. `Edit` src/Article/Content.elm
5. `Read` src/Article.elm
6. `Edit` src/Article.elm
7. `Read` src/Page/Article.elm
8. `Edit` src/Page/Article.elm
9. `Edit` src/Page/Article.elm
10. `Read` src/Page/Article/Editor.elm
11. `Edit` src/Page/Article/Editor.elm
12. `Edit` src/Page/Article/Editor.elm
13. `Bash` rm src/Article/Body.elm 
14. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

#### ✅ 03-extract-module

1. `Read` src/Api.elm
2. `Grep` import Api in src
3. `Read` elm.json
4. `Write` src/Api/Cred.elm
5. `Edit` src/Api.elm
6. `Read` src/Api.elm
7. `Edit` src/Api/Cred.elm
8. `Edit` src/Api/Cred.elm
9. `Edit` src/Api.elm
10. `Edit` src/Api.elm
11. `Edit` src/Api.elm
12. `Read` src/Api.elm
13. `Read` src/Api/Cred.elm
14. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 
15. `Edit` src/Api.elm
16. `Agent` Read import lines from multiple Elm files [Explore]
17. `Read` src/Page.elm
18. `Read` src/Article/Tag.elm
19. `Read` src/Article/Comment.elm
20. `Read` src/Page/Home.elm
21. `Read` src/Main.elm
22. `Read` src/Page/Article.elm
23. `Read` src/Session.elm
24. `Read` src/Profile.elm
25. `Read` src/Viewer.elm
26. `Read` src/Article.elm
27. `Read` src/Author.elm
28. `Read` src/Article/Feed.elm
29. `Read` src/Page/Login.elm
30. `Read` src/Page/Register.elm
31. `Read` src/Page/Profile.elm
32. `Read` src/Page/Settings.elm
33. `Read` src/Page/Article/Editor.elm
34. `Grep` Api\. in src/Page.elm
35. `Grep` Api\. in src/Article/Tag.elm
36. `Grep` Api\. in src/Article/Comment.elm
37. `Grep` Api\. in src/Page/Home.elm
38. `Grep` Api\. in src/Main.elm
39. `Grep` Api\. in src/Page/Article.elm
40. `Grep` Api\. in src/Session.elm
41. `Grep` Api\. in src/Profile.elm
42. `Grep` Api\. in src/Viewer.elm
43. `Grep` Api\. in src/Article.elm
44. `Grep` Api\. in src/Author.elm
45. `Grep` Api\. in src/Article/Feed.elm
46. `Grep` Api\. in src/Page/Login.elm
47. `Grep` Api\. in src/Page/Register.elm
48. `Grep` Api\. in src/Page/Profile.elm
49. `Grep` Api\. in src/Page/Settings.elm
50. `Grep` Api\. in src/Page/Article/Editor.elm
51. `Grep` Api\. in src/Article/Tag.elm
52. `Grep` Api\. in src/Article/Comment.elm
53. `Grep` Api\. in src/Page/Home.elm
54. `Grep` Api\. in src/Main.elm
55. `Grep` Api\. in src/Page/Article.elm
56. `Grep` Api\. in src/Session.elm
57. `Grep` Api\. in src/Viewer.elm
58. `Grep` Api\. in src/Article.elm
59. `Grep` Api\. in src/Author.elm
60. `Grep` Api\. in src/Article/Feed.elm
61. `Grep` Api\. in src/Page/Login.elm
62. `Grep` Api\. in src/Page/Register.elm
63. `Grep` Api\. in src/Page/Profile.elm
64. `Grep` Api\. in src/Page/Settings.elm
65. `Grep` Api\. in src/Page/Article/Editor.elm
66. `Edit` src/Page.elm
67. `Edit` src/Viewer.elm
68. `Edit` src/Article/Feed.elm
69. `Read` src/Page.elm
70. `Read` src/Viewer.elm
71. `Read` src/Article/Feed.elm
72. `Read` src/Main.elm
73. `Read` src/Session.elm
74. `Read` src/Profile.elm
75. `Read` src/Author.elm
76. `Read` src/Article.elm
77. `Read` src/Article/Tag.elm
78. `Read` src/Article/Comment.elm
79. `Read` src/Page/Home.elm
80. `Read` src/Page/Article.elm
81. `Read` src/Page/Login.elm
82. `Read` src/Page/Register.elm
83. `Read` src/Page/Profile.elm
84. `Read` src/Page/Settings.elm
85. `Read` src/Page/Article/Editor.elm
86. `Grep` Api\. in src/Profile.elm
87. `Edit` src/Page.elm
88. `Edit` src/Viewer.elm
89. `Edit` src/Article/Feed.elm
90. `Edit` src/Profile.elm
91. `Edit` src/Main.elm
92. `Edit` src/Session.elm
93. `Edit` src/Author.elm
94. `Edit` src/Article.elm
95. `Edit` src/Article/Tag.elm
96. `Edit` src/Article/Comment.elm
97. `Edit` src/Page/Home.elm
98. `Edit` src/Page/Article.elm
99. `Edit` src/Page/Login.elm
100. `Edit` src/Page/Register.elm
101. `Edit` src/Page/Profile.elm
102. `Edit` src/Page/Settings.elm
103. `Edit` src/Page/Article/Editor.elm
104. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 
105. `Edit` src/Viewer.elm
106. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 
107. `Edit` src/Article/Feed.elm
108. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

#### ✅ 04-add-route

1. `Read` src/Route.elm
2. `Read` src/Page.elm
3. `Read` src/Main.elm
4. `Glob` src/Page/Bookmarks.elm
5. `Read` src/Page/Bookmarks.elm
6. `Write` src/Page/Drafts.elm
7. `Edit` src/Route.elm
8. `Edit` src/Route.elm
9. `Edit` src/Route.elm
10. `Edit` src/Page.elm
11. `Edit` src/Page.elm
12. `Edit` src/Page.elm
13. `Edit` src/Main.elm
14. `Edit` src/Main.elm
15. `Edit` src/Main.elm
16. `Edit` src/Main.elm
17. `Edit` src/Main.elm
18. `Edit` src/Main.elm
19. `Edit` src/Main.elm
20. `Edit` src/Main.elm
21. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

#### ✅ 05-add-variant

1. `Read` src/Page/Home.elm
2. `Edit` src/Page/Home.elm
3. `Edit` src/Page/Home.elm
4. `Edit` src/Page/Home.elm
5. `Edit` src/Page/Home.elm
6. `Bash` elm make src/Main.elm 2>&1 

### treatment (2026-04-10T081651-treatment-5)

#### ✅ 01-add-feature

1. `Bash` elmq list src/Route.elm src/Page.elm src/Main.elm 
2. `Bash` elmq get -f src/Route.elm Route parser routeToString routeToPieces -f src/Page.elm Page viewMenu isA...
3. `Bash` elmq get -f src/Main.elm Model Msg changeRouteTo update view subscriptions toSession 
4. `Bash` elmq list src/Page/Settings.elm 
5. `Bash` elmq get -f src/Page/Settings.elm init toSession subscriptions 
6. `Write` src/Page/Bookmarks.elm
7. `Bash` \ elmq patch --old ' | NewArticle | EditArticle Slug' --new ' | NewArticle | EditArticle Slug | Book...
8. `Bash` \ elmq patch --old ' | NewArticle' --new ' | NewArticle | Bookmarks' src/Page.elm Page && \ elmq pat...
9. `Bash` \ elmq import add src/Main.elm 'Page.Bookmarks as Bookmarks' && \ elmq patch --old ' | Editor (Maybe...
10. `Bash` \ elmq patch --old ' Just (Route.Article slug) -> Article.init session slug |> updateWith Article Go...
11. `Bash` \ elmq patch --old ' ( GotEditorMsg subMsg, Editor slug editor ) -> Editor.update subMsg editor |> u...
12. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 02-rename-module

1. `Bash` elmq mv src/Article/Body.elm src/Article/Content.elm 
2. `Bash` elm make src/Main.elm 2>&1 | tail -5 

#### ✅ 03-extract-module

1. `Bash` elmq list src/Api.elm 
2. `Bash` elmq refs src/Api.elm Cred username credHeader credDecoder 
3. `Bash` elmq move-decl src/Api.elm --to src/Api/Cred.elm Cred username credHeader credDecoder 
4. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 04-add-route

1. `Bash` elmq list src/Route.elm src/Page.elm src/Main.elm 
2. `Bash` elmq get -f src/Route.elm Route parser routeToString routeToPieces -f src/Page.elm Page viewMenu isA...
3. `Bash` elmq get -f src/Main.elm Model Msg toSession changeRouteTo update subscriptions 
4. `Bash` cat src/Page/Bookmarks.elm 
5. `Write` src/Page/Drafts.elm
6. `Bash` \ elmq variant add src/Route.elm --type Route 'Drafts' && \ elmq patch --old ' , Parser.map Bookmark...
7. `Bash` elmq variant add src/Page.elm --type Page 'Drafts' \ --fill 'isActive=( Drafts, Route.Drafts ) -> Tr...
8. `Bash` elmq patch --old ' ( Bookmarks, Route.Bookmarks ) -> True _ ->' --new ' ( Bookmarks, Route.Bookmarks...
9. `Bash` elmq patch --old ' [ linkTo Route.NewArticle [ i [ class "ion-compose" ] [], text "\u{00A0}New Post"...
10. `Bash` elmq variant add src/Main.elm --type Model 'Drafts Drafts.Model' \ --fill 'toSession=Drafts drafts -...
11. `Bash` elmq variant add src/Main.elm --type Msg 'GotDraftsMsg Drafts.Msg' 
12. `Bash` elmq import add src/Main.elm 'Page.Drafts as Drafts' 
13. `Bash` elmq get -f src/Main.elm changeRouteTo update view subscriptions toSession 
14. `Bash` elmq patch --old ' ( _, Drafts _ ) -> Debug.todo "Drafts" ( GotDraftsMsg _, _ ) -> Debug.todo "GotDr...
15. `Bash` elm make src/Main.elm 2>&1 
16. `Bash` elmq patch --old ' Drafts -> [ "drafts" ] Drafts -> Debug.todo "Drafts"' --new ' Drafts -> [ "drafts...
17. `Bash` elm make src/Main.elm 2>&1 
18. `Bash` elmq patch --old ' ( GotSession session, Redirect _ ) -> ( Redirect session , Route.replaceUrl (Sess...
19. `Bash` elm make src/Main.elm 2>&1 

#### ✅ 05-add-variant

1. `Bash` elmq list src/Page/Home.elm 
2. `Bash` elmq get -f src/Page/Home.elm FeedTab viewTabs fetchFeed init 
3. `Bash` elmq variant cases src/Page/Home.elm --type FeedTab 
4. `Bash` elmq get -f src/Page/Home.elm yourFeed globalFeed tagFeed 
5. `Bash` elmq variant add src/Page/Home.elm --type FeedTab 'BookmarkedFeed Cred' \ --fill 'viewTabs=Bookmarke...
6. `Bash` elmq get -f src/Page/Home.elm viewTabs 
7. `Bash` elmq patch --old 'YourFeed cred -> Feed.viewTabs [] (yourFeed cred) [ globalFeed ]' \ --new 'YourFee...
8. `Bash` elmq set src/Page/Home.elm << 'ELM' bookmarkedFeed : Cred -> ( String, Msg ) bookmarkedFeed cred = (...
9. `Bash` elm make src/Main.elm --output=/dev/null 2>&1 

