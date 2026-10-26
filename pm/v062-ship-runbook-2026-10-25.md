# Runbook phát hành v0.6.2 — 25/10/2026 (22:45, giờ VN)

## 1. Blocker #1157: nguyên nhân

**Đây là bug thật trong hook prompt của Claude khi chạy bằng `dash`. Không phải do CI chạy bằng root.**

Log của run lỗi gần nhất: [40191057983](https://github.com/jangtrinh/design-os-studio/actions/runs/40191057983). Run này chạy trên nhánh `fix/1072-private-hook-outputs` @ `6f2f9a7` (PR #1085):
```
panicked at crates/designos-app/src/terminal/unwritable_hook_folder_tests.rs:83:13:
ClaudeAsk, ReadOnly: 1 of 20 hooks did not end at once with success:
exit Some(2) after 8.693598ms, stderr "": { p=$(tr '\r\n' '  '); ... "e":"UserPromptSubmit" ...
test result: FAILED. 3284 passed; 1 failed; 11 ignored
```

**Tại sao không phải do root:**
- Test đã có sẵn `if ReadOnly && running_as_root() { continue; }`.
- Runner chạy bằng user `runner`, không phải root, nên mới vào được nhánh ReadOnly và fail ở đó.

**Nguyên nhân gốc:**
- Hook tạo marker bằng `{ : > marker; } 2>/dev/null || true`.
- Trên `dash` (`/bin/sh` của Ubuntu), khi redirect của builtin đặc biệt `:` bị lỗi thì cả shell thoát với mã 2, có ngoặc nhọn hay không cũng vậy.
- Trên macOS, `sh` là bash nên lỗi không hiện ra.
- Hook `UserPromptSubmit` thoát mã 2 là Claude chặn prompt. Đây là bug người dùng gặp thật.

**Fix nhỏ nhất đã có trên #1085, commit `1966880`:**
```diff
- {{ : > {marker}; }} 2>/dev/null || true
+ {{ true > {marker}; }} 2>/dev/null || true
```
Commit này cũng sửa test để mọi hook được chạy qua cả `sh` lẫn `/bin/dash` (`shells_here()`).

**Kết quả CI:** trên head `beafe3e` ([run 40197935120](https://github.com/jangtrinh/design-os-studio/actions/runs/40197935120)), Ubuntu ✅, macOS ✅, Windows ❌ (#1158).

**Phát hiện quan trọng:** file test này **không có trên `release/v0.6.2`**, nó đi vào cùng #1085. Head của nhánh release là `184e145` (#1135): Ubuntu ✅, macOS ✅. Vì vậy #1157 chỉ chặn #1085, **không chặn việc tag v0.6.2**.

**Đã làm trên #1157:**
- Đã comment chẩn đoán [PM]: https://github.com/jangtrinh/design-os-studio/issues/1157#issuecomment-6399593915
- Đã gắn nhãn `cần trong hôm nay`. Nhãn này chưa có trong repo, PM vừa tạo.
- Giữ nguyên P0 và for:claude.

## 2. Các PR đang mở vào release/v0.6.2

| PR | Draft | Diff | Sửa gì | CI (Ubuntu / macOS / Windows) | Xung đột | Đề xuất |
|---|---|---|---|---|---|---|
| #1085 | không | +49 965/−336, 217 file (phần lớn là log bằng chứng) | #1072: hook ghi qua link trong session folder (bảo mật) | ✅ / ✅ / ❌ | không | **Dời sang v0.6.3.** Lý do: #1072 thuộc milestone v0.7, PR quá lớn, QA chưa smoke, và chạy lại CI mất khoảng 40 phút. Nếu Jang muốn đưa vào thì xem phương án B ở mục 4. |
| #1114 | không | +612, 18 file, chỉ tài liệu | spec v0.6 + runbook phát hành | ❌ (run bị huỷ / runner tắt) / ✅ / ❌ | không | Merge vào main **sau** khi tag. Không ảnh hưởng đến binary. |
| #1130 | có | +718, 7 file | #985: chặn xoá layer trong instance | ❌ (runner tắt) / ✅ / ❌ | không | Dời sang v0.7.1 (milestone của #985). |
| #1117 | có | +5 065/−40, 25 file | `--screenshot` hỏng, lỗi spec dễ đọc hơn | ✅ / ✅ / ❌ | không | Dời sang v0.6.3. Đang là draft và quá lớn để làm trong hôm nay. |
| #1051 | có | +6 276, 63 file | #930: phê duyệt Codex (tính năng mới) | ❌ / ✅ / ❌ | không | Dời sang v0.7 (milestone của #930). |

## 3. Issue còn mở trong milestone v0.6.2

Không phải dời issue nào sang v0.6.3. Không có [SHIP] log cho việc dời.

| Issue | Đã sửa bởi | Việc cần làm |
|---|---|---|
| #1008 | #1103 (đã merge) | Đóng sau khi phát hành |
| #1087 | #1103 (đã merge) | Đóng sau khi phát hành |
| #1036 | #1041 (đã merge) | Đóng sau khi phát hành |
| #1044 | #1045 (đã merge) | Đóng sau khi phát hành |
| #1046 | #1045 (đã merge) | Đóng sau khi phát hành |
| #1053 | #1045 (đã merge; đã bỏ tên tài liệu khỏi file tự nạp) | Đóng sau khi phát hành |
| #1068 | #1071 (merged, contained in 184e145) | Close after release |
| #1061 | #1067 (merged, contained in 184e145) | Close after release |
| #1057 | #1060 (merged, contained in 184e145) | Close after release |
| #1023 | #1035 (merged, contained in 184e145) | Close after release |
| #1039 | #1092 (merged, contained in 184e145) | Close after release |
| #1027 | #1092 (merged, contained in 184e145) | Close after release |
| #1157 | fix nằm trên #1085 | Giữ mở. Nếu đi theo phương án A thì để Jang quyết có dời sang v0.6.3 cùng #1085 không. PM chưa dời vì brief yêu cầu giữ. |

Các issue này đóng không tự động vì PR merge vào nhánh release, không vào nhánh mặc định.

## 4. Các bước cho Jang

**Rủi ro cần Jang chấp nhận:** Windows đỏ trên **mọi** lần merge vào release/v0.6.2 (#1103, #1116, #1135). Đây là #1158 (stack overflow trong test, CRLF), có từ trước và thuộc test harness, không phải lỗi trong build. Workflow Release vẫn build được Windows: lần dry-run 40188357523 lúc 18:19 hôm nay xanh.

**Phương án A (khuyến nghị, phát hành trong khoảng 30 phút):**
1. Không merge thêm PR nào vào release/v0.6.2.
2. Tag đúng commit `184e14535b6092660deda2f04381aa9976acdc56` (head của `release/v0.6.2`, #1135):
   - `Cargo.toml [workspace.package] version = "0.6.2"` ✅
   - `release.yml` chạy khi push tag `v*`, và có job `version-check` kiểm tag khớp với Cargo.
   ```
   git fetch origin && git tag -a v0.6.2 184e14535b6092660deda2f04381aa9976acdc56 -m "v0.6.2" && git push origin v0.6.2
   ```
3. Chờ workflow Release xanh, kiểm tra asset của cả 3 hệ điều hành, rồi publish release.
4. Merge `release/v0.6.2` vào `main`, như các bản trước (v0.6.1 được tag trên main). Sau đó merge #1114 vào main.
5. Đổi base của #1085, #1117, #1130, #1051 sang nhánh v0.6.3 hoặc main.
6. Đóng 12 issue: #1008, #1087, #1036, #1044, #1046, #1053, #1068, #1061, #1057, #1023, #1039, #1027.

**Phương án B (có thêm #1085, khoảng +1 giờ):** merge #1085, chờ CI chạy lại (Ubuntu và macOS phải xanh), tag head mới, rồi đóng thêm #1072 và #1157.

### Nháp release notes v0.6.2
```
## design:os studio v0.6.2

### An toàn agent (Ask mode)
- Quyền phê duyệt Ask-mode nằm ngoài mọi thư mục agent ghi được (#1035)
- Hook của Claude đọc từ thư mục riêng; không nạp settings project/local (#1041, #1045)
- Session folder: không tạo hay ghi xuyên qua link, không chờ FIFO (#1060, #1067, #1071)
- Tab Grok/Antigravity Ask chỉ khởi động khi config đúng của Studio (#1078)
- MCP: siết đường dẫn output, kiểm tra tên, không lộ đường dẫn ngoài workspace (#1103)

### Tính năng
- Tạo text style từ mục Text (#1024)
- Trạng thái phê duyệt của Gemini trong panel (#1058)

### Sửa lỗi
- Dev mode: code giữ trên một dòng, prompt MCP hiện đầy đủ (#1033)
- Export SwiftUI escape chuỗi đúng; thêm `designos-diff --version` (#1090)
- Giữ tên layer của instance khi đồng bộ main component (#1091)
- Properties: hiện library styles, export trang, pill typography (#1092)

### Hiệu năng
- Ghi thay đổi transaction không qua JSON (#1025)

### Known issues
- Test Windows đang đỏ trên CI do lỗi tràn stack trong test (`STATUS_STACK_OVERFLOW`), không phải lỗi build. Bản build Windows vẫn được tạo bình thường. Đang sửa trong #1158 (https://github.com/jangtrinh/design-os-studio/issues/1158).

Issue đã đóng: #1008 #1087 #1036 #1044 #1046 #1053 #1068 #1061 #1057 #1023 #1039 #1027
```
