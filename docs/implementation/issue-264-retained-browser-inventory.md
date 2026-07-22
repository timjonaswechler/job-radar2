# Issue 264 retained Browser inventory

Generated after B01 from the ticket's old-seam/caller/fake searches. Every retained hit is intentionally owned by a later slice:

- **A02** — productive `ProfileBrowserClient` routes, exports, adapters, conveniences, and their phase/application fakes/tests.
- **B02** — managed Chromiumoxide runtime DTOs, lifecycle implementation, and lifecycle tests.

B01 adds a sibling Browser Acquisition contract and does not bridge or delete these hits.

- [A02] `src-tauri/src/app/commands.rs:430,449,468,494,514,1128,1229,1255,1259,1262,1267,1268,1275`
- [B02] `src-tauri/src/browser_runtime/control.rs:13,14,37,40,42,48,49,52,53,58,61,62,157,159,165,167,168,179,193,200,201,228,242,243,259,267,268,282,296,297,322,323,337,338,354,383,385,389,390,394,398,401,402,416,417,423,424,443,444,457,460,461,467,470,492,495,496,532,533,536,537,541,542`
- [B02] `src-tauri/src/browser_runtime/mod.rs:19,30,31`
- [B02] `src-tauri/src/browser_runtime/tests.rs:303,306,317,324,325`
- [B02] `src-tauri/src/browser_runtime/types.rs:81,115,116,120,121,130`
- [A02] `src-tauri/src/checks/mod.rs:26,27,28,29`
- [A02] `src-tauri/src/checks/source_live/activation.rs:5,6,38,43,47,55,60,64,74,90,95,99,107,112,116,126,149`
- [A02] `src-tauri/src/checks/source_live/mod.rs:17,20,34,35,36,62,67,71,79,84,88,98,164`
- [A02] `src-tauri/src/lib.rs:14,15,16,17,135,139,140,150,162`
- [A02] `src-tauri/src/profile_dsl/runtime/browser.rs:9,11,18,26,31,32,36,37,46,56,57,59,61,63,74,82,91,93,94,96,97,105,109,117,118,119,122,124,126,142,143,152,153,161,210,217,223,224,225,231,232,234,235,237,238,240,241,243,245,246,247,249,250,252,253,256`
- [A02] `src-tauri/src/profile_dsl/runtime/detail.rs:38,39,83`
- [A02] `src-tauri/src/profile_dsl/runtime/detail/fetch.rs:24,97,116,130,168,175,178,181`
- [A02] `src-tauri/src/profile_dsl/runtime/detail/strategy.rs:17`
- [A02] `src-tauri/src/profile_dsl/runtime/detail/support.rs:51,58,61,64,68,74,80,84`
- [A02] `src-tauri/src/profile_dsl/runtime/discovery.rs:36,37,82`
- [A02] `src-tauri/src/profile_dsl/runtime/discovery/fetch.rs:29,66,103,143,233,255,272,314,320,321,324`
- [A02] `src-tauri/src/profile_dsl/runtime/discovery/pagination.rs:19`
- [A02] `src-tauri/src/profile_dsl/runtime/discovery/strategy.rs:15,189`
- [A02] `src-tauri/src/profile_dsl/runtime/discovery/support.rs:47,54,57,60,64,70,76,80`
- [A02] `src-tauri/src/profile_dsl/runtime/mod.rs:25,26,27`
- [A02] `src-tauri/src/profile_dsl/runtime/source_detail.rs:16,221,234`
- [A02] `src-tauri/src/search/posting/service.rs:9,136,137,141,150`
- [A02] `src-tauri/src/search/posting/tests.rs:4,5,8,37,38,76,78,91,94,97,111,112,116`
- [A02] `src-tauri/src/search/posting/tests/detail_loading/basic.rs:50,54,214,218,276,280`
- [A02] `src-tauri/src/search/posting/tests/detail_loading/browser.rs:46,55`
- [A02] `src-tauri/src/search/posting/tests/detail_loading/context.rs:70,74`
- [A02] `src-tauri/src/search/posting/tests/detail_loading/diagnostics.rs:57,61,211,215,325,329`
- [A02] `src-tauri/src/search/posting/tests/detail_loading/fallback.rs:73,77,270,274`
- [A02] `src-tauri/src/search/run/execution.rs:9,10,95,108`
- [A02] `src-tauri/src/search/run/tests/support.rs:112,158`
- [A02] `src-tauri/src/source_profile/detection/browser.rs:18,19,22,29,59,91,200,210,285,291,294,297,303,309,312,315`
- [A02] `src-tauri/src/source_profile/detection/mod.rs:10,21,123,132,290,393`
- [A02] `src-tauri/tests/profile_dsl_compiler/resolution.rs:11,100`
- [A02] `src-tauri/tests/profile_dsl_runtime/detail.rs:25,26,29,163,213,868,922,923,1133,1176,1276,1279,1282,1290,1292,1296,1305,1306,1335,1337,1338,1341,1353,1361,1366,1369,1372,1383,1384,1388`
- [A02] `src-tauri/tests/profile_dsl_runtime/discovery.rs:12,13,54,56,57,60,72,80,85,88,91,102,103,107`
- [A02] `src-tauri/tests/profile_dsl_runtime/discovery/cancellation.rs:24,80,146,198,203,209,212,215,221,225,227,231,240,241`
- [A02] `src-tauri/tests/profile_dsl_runtime/discovery/document_types_and_browser.rs:138,220,221`
- [A02] `src-tauri/tests/profile_dsl_runtime/discovery/post_request_bodies.rs:194`
- [A02] `src-tauri/tests/profile_dsl_runtime/strategy_allowances.rs:21,115,153,190,235,263,293,320,352,382,413,450,488,531,567,611,650,679`
- [A02] `src-tauri/tests/profile_dsl_runtime/strategy_set.rs:18,487,536,591,609,649,689,732,764,800,835,872,928,969,994,1024,1072,1114,1160,1179,1217,1235,1271,1296,1330,1388,1435,1478,1534,1560,1593,1653,1704,1747,1770,1809,1844,1879,1909,1945`
- [A02] `src-tauri/tests/source/live_check.rs:4,5,55,123,172,257,311,368,557,588,628,661,733,771,819,836,869,905,935,968,998`
- [A02] `src-tauri/tests/source/profile_detection.rs:4,7,8,114,116,169,171,211,213,323,325,583,585,629,631,1112,1117,1182,1187,1215,1216,1222,1244,1250,1256,1264,1270,1293,1295,1330,1335,1365,1367,1400,1402,1434,1436,1512,1513,1514,1517,1522,1528,1535,1540,1543,1546,1558,1559`
- [A02] `src-tauri/tests/source_detail_execution.rs:13,92,119,190,260,303,398,413,448,505,558,575,610,618`
- [A02] `src-tauri/tests/support/mod.rs:7,137,157,178,215,238,262`
