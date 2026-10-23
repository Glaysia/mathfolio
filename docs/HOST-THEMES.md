# 호스트 테마 연동

2026-10-23. 공통 Rust UI에 `테마 → 호스트 테마 따르기 / 밝게 / 어둡게 / Nord`를 추가했다. 초기 모드는 호스트 따르기다. 호스트 연결 전에는 `configure_surface`에 전달한 기본 밝은/어두운 테마를 쓰며, UI에서도 이 상태를 안내한다. 작은 container에서는 파일 메뉴 안에 테마 선택이 있다.

Obsidian·VS Code용 TS 구현과 설정 저장은 **각 플러그인의 별도 레포**에서 한다. 이 레포에는 CSS/DOM 해석기·플러그인 코드·JS 실행 엔진을 추가하지 않는다. WASM 바인딩·실제 호스트 연결은 아직 미검증이다. 아래는 Rust에서 구현·검사한 계약이다.

## Rust API

공개 API는 `mathcraft_ui::{HostTheme, ThemeMode, set_host_theme, set_theme_mode, theme_mode, has_host_theme}`다. 기존 `set_theme(ctx, Theme::Light/Dark)`는 명시적인 밝게/어둡게 선택으로 유지한다.

```rust
let mut tokens = mathcraft_ui::HostTheme::for_theme(mathcraft_ui::Theme::Dark);
tokens.panel = [30, 30, 30, 255];
tokens.text = [220, 220, 220, 255];
tokens.accent = [0, 180, 150, 255];
tokens.button = [0, 100, 80, 255];
tokens.button_text = [255, 255, 255, 255];
mathcraft_ui::set_host_theme(&ctx, Some(tokens))?;
mathcraft_ui::set_theme_mode(&ctx, mathcraft_ui::ThemeMode::FollowHost);
```

`HostTheme`와 `ThemeMode`는 serde 직렬화를 지원한다. 모드의 wire 값은 `follow_host`, `light`, `dark`, `nord`다. 토큰은 CSS 문자열 대신 sRGB RGBA 배열을 사용한다. workspace/panel/surface/button 배경은 불투명해야 하므로, CSS 투명도는 호스트의 실제 부모 배경에 합성해서 넘긴다. 빠진 호스트 변수는 `HostTheme::for_theme`의 대응 기본값으로 보충한다.

Nord 프리셋은 [공식 팔레트](https://www.nordtheme.com/docs/colors-and-palettes/)의 Polar Night·Snow Storm·Frost·Aurora 색을 UI 의미에 맞게 배치한다. `HostTheme::nord()` 또는 `ThemeMode::Nord`로 사용할 수 있고 호스트 연결 없이 동작한다. 호스트 따르기는 Nord로 강제하지 않고 실제 전달된 색을 적용하므로, Obsidian/VS Code의 Nord 변형이나 사용자 accent도 보존한다. 출처·MIT 고지는 [NOTICE](../NOTICE.md)에 연결했다.

`set_host_theme`는 오류 없이 변경을 저장했으면 `Ok(true)`, 같은 값이면 `Ok(false)`를 반환한다. 잘못된 font size·corner radius·기본 배경 투명도는 기존 상태를 바꾸지 않고 거절한다. UI 글자 크기는 10~24 logical px, 모서리는 0~16 범위다. 테마 갱신은 글꼴 bytes 재설치나 문서 전체 직렬화를 요구하지 않는다.

- 호스트 모드에서는 새 토큰을 즉시 적용한다. 같은 토큰 반복 전달은 style 재설정·추가 repaint를 일으키지 않는다.
- 밝게/어둡게/Nord를 직접 선택한 동안에는 호스트 갱신을 보관하고 현재 UI를 유지한다. 호스트 따르기로 돌아가면 가장 최근 값을 적용한다.
- `set_host_theme(ctx, None)`은 연결을 해제하고 초기 fallback으로 돌아간다. 다른 편집 표면의 테마에는 영향을 주지 않는다.
- `theme_mode`로 선택을 읽을 수 있다. 설정을 저장·복원할 책임은 호스트에 있다. 독립판의 디스크 설정 저장은 이번 변경에 포함하지 않는다.

## 의미별 토큰과 호스트 연결

두 호스트의 공식 색 변수에서 실제 computed 값을 읽어 아래 의미로 전달한다. 이 매핑은 플러그인 구현을 위한 기준이며 아직 실제 연결이 구현됐다는 뜻은 아니다.

| Rust 토큰 | Obsidian 후보 | VS Code Webview 후보 |
| --- | --- | --- |
| workspace | `--background-primary` | `--vscode-editor-background` |
| panel | `--background-secondary` | `--vscode-sideBar-background` |
| surface | `--background-modifier-form-field` | `--vscode-input-background` |
| text / muted_text | `--text-normal` / `--text-muted` | `--vscode-foreground` / `--vscode-descriptionForeground` |
| border / focus | `--background-modifier-border` / `--background-modifier-border-focus` | `--vscode-input-border` / `--vscode-focusBorder` |
| accent | `--text-accent` | `--vscode-textLink-foreground` |
| button / button_text | `--interactive-accent` / `--text-on-accent` | `--vscode-button-background` / `--vscode-button-foreground` |
| hover | `--background-modifier-hover` | `--vscode-toolbar-hoverBackground` |
| selection / selection_text | `--text-selection` / `--text-normal` | `--vscode-list-activeSelectionBackground` / `--vscode-list-activeSelectionForeground` |
| error / warning | `--text-error` / `--text-warning` | `--vscode-errorForeground` / `--vscode-editorWarning-foreground` |

`dark`는 호스트의 테마 종류에서, `high_contrast`는 해당 호스트의 고대비 설정에서 얻는다. UI 크기·모서리·글꼴도 명시적으로 전달한다. CSS `font-family` 이름만으로 Rust 글꼴이 설치되는 것은 아니며, 글꼴 변경은 별도의 적법한 font bytes 공급 경계를 사용한다. 수식용 STIX·문서에 저장된 글꼴 크기·표/수식/텍스트 색과 PDF 출력은 UI 테마로 바꾸지 않는다.

Obsidian은 공식 [`css-change` 이벤트](https://github.com/obsidianmd/obsidian-api/blob/master/obsidian.d.ts)와 [의미별 CSS 변수](https://docs.obsidian.md/Reference/CSS%20variables/About%20styling)를 사용한다. VS Code는 [Webview 테마 변수](https://code.visualstudio.com/api/extension-guides/webview#theming-webview-content)를 읽는다. host theme 변경을 알리고 실제 computed 값이 갱신됐을 때 한 번 전달한다. 매 frame DOM을 읽거나 Canvas/문서를 재생성하지 않는다. 닫힐 때 관련 이벤트·관찰자를 해제한다. 기본·커뮤니티·고대비 테마는 각 플러그인에서 실제 검증한다.

## 검사

Rust 테스트는 custom 색·주 버튼의 전경/배경 분리·고대비 윤곽, 수동 선택 중 최신 host 보존, 연결 해제·fallback, 동일 입력의 무작업, 잘못된 입력의 원자적 거절, 직렬화와 표면 간 격리를 검사한다. application-level 검사는 미확정 수식이 있는 상태의 live theme 변경이 문서 색·수식·Undo·계산 generation·확대율을 바꾸지 않는지 확인한다. 실제 두 호스트에서의 외형 일치는 후속 검증이다.
