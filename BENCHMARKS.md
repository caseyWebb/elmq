============================================
  elmq MCP Benchmark Analysis
============================================

============================================
  Per-scenario detail (averages across runs, winners bolded)
============================================

Pricing: input $3.00/M · output $15.00/M · cache read $0.30/M · cache write $3.75/M (Sonnet 4.6)

SCENARIO             ARM             INPUT     OUTPUT     CACHE_R    CACHE_C       COST   TURNS    TIME    PASS
--------------------------------------------------------------------------------------------------------------
01-add-feature       control           361      5,219     572,793     41,252    $0.4059      23    2:14     5/5
                                                                               ±$0.0404      ±4                
01-add-feature       treatment          47      4,511     537,944     36,949    $0.3677      25    1:49     5/5
                                                                               ±$0.0411      ±2                

02-rename-module     control           172      2,258     214,310     14,567    $0.1533      17    0:39     5/5
                                                                               ±$0.0296      ±2                
02-rename-module     treatment           5        321      46,132     11,037    $0.0601       3    0:08     5/5
                                                                               ±$0.0003      ±0                

03-extract-module    control            33     21,731   1,122,644     46,279    $0.8364      38    5:57     5/5
                                                                               ±$0.3262     ±27                
03-extract-module    treatment          20      6,665     459,878     24,774    $0.3309      19    2:06     5/5
                                                                               ±$0.0547      ±3                

04-add-route         control            22      4,746     538,029     24,155    $0.3232      21    1:45     5/5
                                                                               ±$0.0465      ±1                
04-add-route         treatment          25      4,317     585,892     28,099    $0.3460      25    1:37     5/5
                                                                               ±$0.0536      ±3                

05-add-variant       control             9      2,159     134,692     14,046    $0.1255       7    0:42     5/5
                                                                               ±$0.0057      ±0                
05-add-variant       treatment          14      2,829     253,908     15,643    $0.1773      13    0:53     5/5
                                                                               ±$0.0300      ±2                

--------------------------------------------------------------------------------------------------------------
OVERALL (sum)        control             -          -           -          -    $1.8443     106   11:19   25/25
                     treatment           -          -           -          -    $1.2820      85    6:36   25/25

  Δ cost  (trt − ctrl): $-0.5624 (-30.5%) ± $0.3458
  Δ turns (trt − ctrl): -21 (-19.8%) ± 28

============================================
  Tool Breakdown (per scenario, latest run)
============================================

control (2026-04-10T053333-control-5):

  01-add-feature [PASSED]:
      15  Edit
      13  Read
       2  Write
       2  Bash
       1  ToolSearch
       1  Grep
       1  Glob
       1  Agent

  02-rename-module [PASSED]:
       7  Edit
       5  Read
       5  Bash
       2  Write
       1  ToolSearch
       1  Grep
       1  Glob

  03-extract-module [PASSED]:
      11  Grep
       4  Write
       3  Read
       3  Bash
       2  Glob
       2  Edit
       1  ToolSearch

  04-add-route [PASSED]:
      17  Read
      15  Edit
       2  Write
       2  Bash
       1  ToolSearch
       1  Grep
       1  Glob
       1  Agent

  05-add-variant [PASSED]:
       5  Edit
       2  Read
       2  Bash
       1  Write
       1  ToolSearch
       1  Grep
       1  Glob

treatment (2026-04-10T053333-treatment-5):

  01-add-feature [PASSED]:
      56  Bash
       1  Write
       1  ToolSearch
       1  Read
       1  Grep
       1  Glob
       1  Edit
       1  Agent

  02-rename-module [PASSED]:
       3  Bash
       1  Write
       1  ToolSearch
       1  Read
       1  Grep
       1  Glob
       1  Edit

  03-extract-module [PASSED]:
      18  Bash
       1  Write
       1  ToolSearch
       1  Read
       1  Grep
       1  Glob
       1  Edit

  04-add-route [PASSED]:
      24  Bash
       2  Write
       1  ToolSearch
       1  Read
       1  Grep
       1  Glob
       1  Edit

  05-add-variant [PASSED]:
      16  Bash
       1  Write
       1  ToolSearch
       1  Read
       1  Grep
       1  Glob
       1  Edit

============================================
  Tool Call Details (per scenario, latest run)
============================================

control (2026-04-10T053333-control-5):

  01-add-feature [PASSED]:
      1. Agent        Explore codebase structure [Explore]
      2. Read         src/Route.elm
      3. Read         src/Main.elm
      4. Read         src/Page.elm
      5. Read         src/Page/Settings.elm
      6. Read         src/Route.elm
      7. Read         src/Page.elm
      8. Read         src/Main.elm
      9. Read         src/Page/Settings.elm
     10. Write        src/Page/Bookmarks.elm
     11. Edit         src/Route.elm
     12. Edit         src/Route.elm
     13. Edit         src/Route.elm
     14. Edit         src/Page.elm
     15. Edit         src/Page.elm
     16. Edit         src/Page.elm
     17. Edit         src/Main.elm
     18. Edit         src/Main.elm
     19. Edit         src/Main.elm
     20. Edit         src/Main.elm
     21. Edit         src/Main.elm
     22. Edit         src/Main.elm
     23. Edit         src/Main.elm
     24. Edit         src/Main.elm
     25. Bash         elm make src/Main.elm --output=/dev/null 2>&1 

  02-rename-module [PASSED]:
      1. Bash         grep -rn "Article\.Body" src/ --include="*.elm" 
      2. Bash         ls src/Article/ 
      3. Read         src/Article/Body.elm
      4. Write        src/Article/Content.elm
      5. Bash         rm src/Article/Body.elm 
      6. Read         src/Article.elm
      7. Edit         src/Article.elm
      8. Edit         src/Page/Article.elm
      9. Read         src/Page/Article.elm
     10. Read         src/Page/Article/Editor.elm
     11. Edit         src/Page/Article.elm
     12. Edit         src/Page/Article/Editor.elm
     13. Edit         src/Page/Article.elm
     14. Edit         src/Page/Article/Editor.elm
     15. Bash         elm make src/Main.elm 2>&1 

  03-extract-module [PASSED]:
      1. Read         src/Api.elm
      2. Grep         import Api in src
      3. Grep         Api\.Cred\b|Api exposing.*Cred|Api exposing \(.*Cred in src
      4. Grep         ^import Api in src
      5. Glob         src/Api
      6. Grep         Api\.username\b in src
      7. Grep         Api\.credHeader\b|Api\.credDecoder\b in src
      8. Grep         Api\. in src
      9. Grep         Api\. in src/Page
     10. Grep         Api\. in src/Article
     11. Grep         Api\b in src/Profile.elm
     12. Grep         Api\b in src/Page.elm
     13. Bash         mkdir -p src/Api 
     14. Write        src/Api/Cred.elm
     15. Edit         src/Api.elm
     16. Read         src/Api.elm
     17. Write        src/Api/Cred.elm
     18. Write        src/Api.elm
     19. Bash         elm make src/Main.elm 2>&1 

  04-add-route [PASSED]:
      1. Agent        Explore existing page patterns [Explore]
      2. Read         src/Route.elm
      3. Read         src/Page.elm
      4. Read         src/Main.elm
      5. Read         src/Page/Blank.elm
      6. Read         src/Page/NotFound.elm
      7. Read         src/Page/Settings.elm
      8. Read         src/Page/Bookmarks.elm
      9. Write        src/Page/Drafts.elm
     10. Edit         src/Route.elm
     11. Read         src/Route.elm
     12. Edit         src/Route.elm
     13. Edit         src/Route.elm
     14. Read         src/Page.elm
     15. Edit         src/Page.elm
     16. Edit         src/Page.elm
     17. Edit         src/Page.elm
     18. Read         src/Main.elm
     19. Edit         src/Main.elm
     20. Edit         src/Main.elm
     21. Edit         src/Main.elm
     22. Edit         src/Main.elm
     23. Edit         src/Main.elm
     24. Edit         src/Main.elm
     25. Edit         src/Main.elm
     26. Edit         src/Main.elm
     27. Bash         elm make src/Main.elm 2>&1 

  05-add-variant [PASSED]:
      1. Read         src/Page/Home.elm
      2. Edit         src/Page/Home.elm
      3. Edit         src/Page/Home.elm
      4. Edit         src/Page/Home.elm
      5. Edit         src/Page/Home.elm
      6. Bash         elm make src/Main.elm 2>&1 

treatment (2026-04-10T053333-treatment-5):

  01-add-feature [PASSED]:
      1. Agent        Explore existing page patterns [Explore]
      2. Bash         elmq list src/Route.elm 
      3. Bash         elmq get src/Route.elm Route 
      4. Bash         elmq get src/Route.elm fromUrl href replaceUrl 
      5. Bash         elmq list src/Page.elm 
      6. Bash         elmq get src/Page.elm view 
      7. Bash         elmq list src/Main.elm 
      8. Bash         elmq get src/Main.elm Model 
      9. Bash         elmq get src/Main.elm Msg 
     10. Bash         elmq get src/Main.elm changeRouteTo 
     11. Bash         elmq get src/Main.elm update 
     12. Bash         elmq get src/Main.elm view 
     13. Bash         elmq get src/Main.elm subscriptions 
     14. Bash         elmq get src/Main.elm toSession 
     15. Bash         elmq list src/Page/Settings.elm 
     16. Bash         elmq get src/Page/Settings.elm Model Msg init update view subscriptions toSession 
     17. Bash         elmq get src/Route.elm Route fromUrl href replaceUrl 
     18. Bash         elmq get src/Page.elm view 
     19. Bash         elmq get src/Page.elm viewHeader navbarLink Page 
     20. Bash         elmq get src/Main.elm Model Msg changeRouteTo update view subscriptions toSession 
     21. Bash         elmq get src/Route.elm parser routeToString isActive 
     22. Bash         elmq get src/Page.elm viewMenu isActive 
     23. Bash         elmq get src/Route.elm routeToPieces 
     24. Bash         elmq list src/Main.elm | head -30 
     25. Bash         elmq get src/Page/Settings.elm init toSession subscriptions 
     26. Bash         cat > src/Page/Bookmarks.elm << 'ELM' module Page.Bookmarks exposing (Model, Msg, init, subscription...
     27. Bash         elmq patch --old ' | EditArticle Slug' --new ' | EditArticle Slug | Bookmarks' src/Route.elm Route 
     28. Bash         elmq patch --old ' , Parser.map NewArticle (s "editor")' --new ' , Parser.map Bookmarks (s "bookmark...
     29. Bash         elmq patch --old ' EditArticle slug -> [ "editor", Slug.toString slug ]' --new ' EditArticle slug ->...
     30. Bash         elmq patch --old ' | NewArticle' --new ' | NewArticle | Bookmarks' src/Page.elm Page 
     31. Bash         elmq patch --old ' [ linkTo Route.NewArticle [ i [ class "ion-compose" ] [], text "\u{00A0}New Post"...
     32. Bash         elmq patch --old ' ( NewArticle, Route.NewArticle ) -> True _ ->' --new ' ( NewArticle, Route.NewArt...
     33. Bash         elmq import add src/Main.elm 'Page.Bookmarks as Bookmarks' 
     34. Bash         elmq patch --old ' | Editor (Maybe Slug) Editor.Model' --new ' | Editor (Maybe Slug) Editor.Model | ...
     35. Bash         elmq patch --old ' | GotEditorMsg Editor.Msg' --new ' | GotEditorMsg Editor.Msg | GotBookmarksMsg Bo...
     36. Bash         elmq patch --old ' Just (Route.Article slug) -> Article.init session slug |> updateWith Article GotA...
     37. Bash         elmq patch --old ' ( GotEditorMsg subMsg, Editor slug editor ) -> Editor.update subMsg editor |> upd...
     38. Bash         elmq patch --old ' Editor Nothing editor -> viewPage Page.NewArticle GotEditorMsg (Editor.view edito...
     39. Bash         elmq patch --old ' Editor _ editor -> Sub.map GotEditorMsg (Editor.subscriptions editor)' --new ' Ed...
     40. Bash         elmq patch --old ' Editor _ editor -> Editor.toSession editor' --new ' Editor _ editor -> Editor.toS...
     41. Bash         elm make src/Main.elm 2>&1 

  02-rename-module [PASSED]:
      1. Bash         elmq mv src/Article/Body.elm src/Article/Content.elm 
      2. Bash         elm make src/Main.elm 2>&1 | tail -5 

  03-extract-module [PASSED]:
      1. Bash         elmq list src/Api.elm 
      2. Bash         elmq get src/Api.elm Cred username credHeader credDecoder 
      3. Bash         elmq refs src/Api.elm Cred username credHeader credDecoder 
      4. Bash         elmq move-decl src/Api.elm --to src/Api/Cred.elm Cred username credHeader credDecoder --dry-run 2>&1...
      5. Bash         elmq move-decl src/Api.elm --to src/Api/Cred.elm Cred username credHeader credDecoder 2>&1 
      6. Bash         elm make src/Main.elm 2>&1 
      7. Bash         elmq get src/Api.elm storeCredWith 
      8. Bash         elmq grep "import Api.Cred" src/Api.elm 
      9. Bash         elmq list src/Api/Cred.elm 
     10. Bash         elmq list src/Api.elm 2>&1 | head -20 
     11. Bash         elmq import add src/Api/Cred.elm 'Json.Encode as Encode exposing (Value)' 
     12. Bash         elmq set src/Api/Cred.elm << 'ELM' encodeCredFields : Cred -> List ( String, Encode.Value ) encodeCr...
     13. Bash         elmq expose src/Api/Cred.elm encodeCredFields 
     14. Bash         elmq get src/Api.elm storeCredWith 
     15. Bash         elmq import add src/Api.elm 'Api.Cred exposing (Cred, credDecoder, credHeader, encodeCredFields)' 
     16. Bash         elmq patch --old 'storeCredWith (Cred uname token) avatar = let json = Encode.object [ ( "user" , En...
     17. Bash         elm make src/Main.elm 2>&1 

  04-add-route [PASSED]:
      1. Bash         elmq list src/Route.elm src/Page.elm src/Main.elm 
      2. Bash         elmq get src/Route.elm Route parser routeToString routeToPieces && elmq get src/Page.elm Page viewMe...
      3. Bash         elmq get src/Main.elm Model Msg toSession changeRouteTo update subscriptions 
      4. Bash         cat -n src/Page/Bookmarks.elm 
      5. Bash         elmq get src/Main.elm view 
      6. Write        src/Page/Drafts.elm
      7. Bash         elmq variant add src/Route.elm --type Route 'Drafts' 
      8. Bash         elmq patch --old ', Parser.map Bookmarks (s "bookmarks")' --new ', Parser.map Bookmarks (s "bookmark...
      9. Bash         elmq patch --old ' Drafts -> Debug.todo "routeToPieces"' --new ' Drafts -> [ "drafts" ]' src/Route.e...
     10. Bash         elmq get src/Route.elm routeToPieces 
     11. Bash         elmq patch --old ' Drafts -> Debug.todo "Drafts"' --new ' Drafts -> [ "drafts" ]' src/Route.elm rout...
     12. Bash         elmq variant add src/Page.elm --type Page 'Drafts' 
     13. Bash         elmq patch --old ' [ linkTo Route.Bookmarks [ i [ class "ion-bookmark" ] [], text "\u{00A0}Bookmarks...
     14. Bash         elmq patch --old ' ( Bookmarks, Route.Bookmarks ) -> True _ ->' --new ' ( Bookmarks, Route.Bookmarks...
     15. Bash         elmq import add src/Main.elm 'Page.Drafts as Drafts' 
     16. Bash         elmq variant add src/Main.elm --type Model 'Drafts Drafts.Model' 
     17. Bash         elmq variant add src/Main.elm --type Msg 'GotDraftsMsg Drafts.Msg' 
     18. Bash         elmq get src/Main.elm toSession changeRouteTo update subscriptions view 
     19. Bash         elmq patch --old ' Drafts _ -> Debug.todo "Drafts"' --new ' Drafts drafts -> Drafts.toSession drafts...
     20. Bash         elmq patch --old ' Drafts _ -> Debug.todo "Drafts"' --new ' Drafts drafts -> Sub.map GotDraftsMsg (D...
     21. Bash         elmq patch --old ' Drafts _ -> Debug.todo "Drafts"' --new ' Drafts drafts -> viewPage Page.Drafts Go...
     22. Bash         elmq patch --old ' ( _, _ ) -> -- Disregard messages that arrived for the wrong page. ( model, Cmd.n...
     23. Bash         elmq patch --old ' Just Route.Bookmarks -> Bookmarks.init session |> updateWith Bookmarks GotBookmar...
     24. Bash         elm make src/Main.elm --output /dev/null 2>&1 

  05-add-variant [PASSED]:
      1. Bash         elmq variant cases src/Page/Home.elm --type FeedTab 
      2. Bash         elmq variant add src/Page/Home.elm --type FeedTab 'BookmarkedFeed Cred' \ --fill 'viewTabs=Bookmarke...
      3. Bash         elmq grep --definitions --source 'yourFeed\|globalFeed\|tagFeed' src/Page/Home.elm 
      4. Bash         elmq grep 'yourFeed\|globalFeed' src/Page/Home.elm 
      5. Bash         elmq grep 'yourFeed' src/Page/Home.elm 
      6. Bash         elmq get src/Page/Home.elm yourFeed globalFeed 
      7. Bash         elmq set src/Page/Home.elm << 'ELM' bookmarkedFeed : Cred -> ( String, Msg ) bookmarkedFeed cred = (...
      8. Bash         elmq get src/Page/Home.elm viewTabs 
      9. Bash         elmq patch --old ' YourFeed cred -> Feed.viewTabs [] (yourFeed cred) [ globalFeed ]' \ --new ' YourF...
     10. Bash         elmq patch \ --old ' Just cred -> [ yourFeed cred ] Nothing -> [] in Feed.viewTabs otherTabs globalF...
     11. Bash         elmq patch \ --old ' BookmarkedFeed cred -> let otherTabs = case maybeCred of Just c -> [ yourFeed c...
     12. Bash         elmq patch \ --old ' Just cred -> [ yourFeed cred, globalFeed ] Nothing -> [ globalFeed ] in Feed.vi...
     13. Bash         elmq grep 'YourFeed\|ClickedTab\|feedTab' src/Page/Home.elm 
     14. Bash         elmq grep 'YourFeed' src/Page/Home.elm 
     15. Bash         elm make src/Main.elm 2>&1 

