# 문단 편집·정렬: r214 고정 범위

원본/GUI/CLR 실행 없이 원본 정적 계약과 자체 Rust 경로를 대조했다. 검사 수·부분 구현 깊이를 완성률 또는40% 달성으로 환산하지 않는다.

앱은 같은 문단의 엄격한 Run 내부 endpoint 선택 삭제만 연결한다. 비어 있지 않은 선택 치환은 source metadata·언어·서식·입력 문맥을 검사하는 core API만 있으며 앱은 미지원이다. Backspace의 제한된 후속 owner는 일반 paste/IME affinity가 아니다. Run 경계/전체 문단 선택 삭제·임의 다음 typing·문단 병합/LineBreak는 미완료다.

Left/Center/Right/Justify는 한 admitted LTR Paragraph의 명시 setter다. source-bound cached context, FIFO prefix/command/suffix, collapsed caret, Undo/Redo와 own/native cold를 연결했다. typography job·TextEdit의 같은 정렬 경로와 기존 실제 pointer 회귀를 유지했다.

authored canonical Left 복원은 자체 scaffold 정책이며 기존 Enter를 회복한다. authored nonLeft Enter는 원자적으로 거절한다. native에서 새로 추가한 Paragraph alignment override는 Left로 바꿔도 explicit override가 남아 기존 mapped split proof와 합성되지 않는다. 원본 serializer 동등성으로 일반화하지 않는다.

명시 U toolbar는 imported absent/Unknown에도 canonical None/Underline을 쓸 수 있다. 조회를 None으로 추정하지 않으며 Unknown Ctrl+U는 계속 예약/no-change다.

실제 ribbon pointer 첫 검사는 authored Left 복귀 뒤 Enter가 기존 scaffold proof에 막히는 제품 버그를 드러냈다. authored canonical Left 정규화 제품 수리로 기존 Enter를 회복했으며, 동일한 pointer test를 변경하지 않고 통과했다. 원래 native Paragraph 속성 권한은 바꾸지 않았다. 제품 수리 receipt와 실패/재검사 로그를 보존한다.

이와 별개로 초기 alignment 앱 검사 두 실패는 실제 sample01의 inherited root Left와 기존 standalone CtrlR preedit 취소 정책에 맞지 않은 fixture 기대였다. 원문 실패/수정 검사 로그를 보존하며 위 제품 수리와 섞지 않는다.

[원본 문단 정렬 BAML/CIL](../../artifacts/rea/paragraph-alignment-next-20261020.md), [명시 U setter 근거](../../artifacts/rea/underline-toolbar-explicit-next-20261020.md), [cross-Run 정적 계약](../../artifacts/rea/rich-text-multirun-next-20261020.md)·[선택 삭제 후속 입력 근거](../../artifacts/rea/rich-text-postdelete-next-20261020.md)를 구분한다.

## 고정 검증

| 항목 | 결과 |
|---|---|
| windows | 1920 |
| peets | 1921 |
| ignored_windows | 100 |
| ignored_peets | 98 |
| mcdx | 192 |
| standalone_xml | 10 |
| pdf | 120 |
| xsd_parts | 1162 |
| underline_own | 12 |
| underline_observations | 2 |
| archive SHA | `d0e759b76b69a8a3e193aea184afc3b00c4c17db1271953c3af15db82b7d22ef` |
| manifest SHA | `edee29964b8c054bf06fc4005acab71a7ee1707cce39bf2251f2feced092404b` |
| accepted validation SHA | `7f5dc244a5d9e93039341757b29dd1d10be84ab75e47c7dd8547665a4477d06f` |

192 MCDX+10XML/120PDF는 r213의 기존 독립 packet 범위 그대로다. 새 alignment normal 테스트의 PDF export를 새 독립 semantic PDF 상태로 세지 않는다. 원본 runtime 재열기·WPF pixel parity·GUI 성능·40% 목표는 미완료다.

- [accepted validation·전체 이전 retry 이력](../../artifacts/rea/paragraph-edit-integration-20261020-r214/validation.json)
- [초기 alignment 앱 fixture 실패](../../artifacts/rea/paragraph-alignment-app-r214-first-preflight-20261020.log)
- [fixture 기대 정정 재검사](../../artifacts/rea/paragraph-alignment-app-r214-test-repair-preflight-20261020.log)
- [실제 pointer 첫 제품 실패](../../artifacts/rea/paragraph-alignment-ribbon-pointer-r214-first-preflight-20261020.log)
- [authored Left 제품 수리 receipt](../../artifacts/rea/authored-left-restore-r214-adoption-20261020.json)
- [authored Left 복원 세 core 회귀](../../artifacts/rea/authored-left-restore-r214-core-preflight-20261020.log)
- [변경하지 않은 실제 네 정렬 pointer 회귀 재검사](../../artifacts/rea/paragraph-alignment-ribbon-pointer-r214-left-repair-preflight-20261020.log)
- [기존 metadata/fmt 이력](0201-underline-and-readonly-save.md)
