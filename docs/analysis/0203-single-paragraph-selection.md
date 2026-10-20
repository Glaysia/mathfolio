# 단일 문단 선택 편집: r216 고정 범위

이 문서는 채택된 r216 고정 검증의 범위를 기록하며, 원본/GUI/CLR 실행이나 검사 수를 근거로 완성률·40% 달성을 주장하지 않는다. r214의 문단 정렬·서식 범위는 [이전 고정 기록](0202-paragraph-edit-and-alignment.md)으로 보존한다.

같은 문단의 cross-Run 선택 **삭제**에 Run/Paragraph 경계와 전체 문단 범위를 연결했다. 선택된 문자 body만 비우고 root·Paragraph·Run wrapper·style key·선택 밖 원문을 보존한다. 여러 문단의 구분자를 지우거나 임의의 새 문자를 넣는 일반 치환으로 확대하지 않는다. 삭제만으로 후속 typing owner를 임의 선택하지 않는다.

실제 Ctrl+A 뒤 한 번의 ordinary Text는 비어 있지 않은 단일 direct-Run Paragraph의 별도 흐름이다. authored/native 모두 첫 nonempty Run을 사용하며, 선행 empty Run이나 seed/default·마지막 Run으로 돌아가지 않는다. 실제 명령·관찰된 전체 선택·owner·정확한 source version·포커스와 한 번의 Text 전달에 임시 권한을 묶고, 물리 문자 key+Text/FIFO 순서를 유지한다. unknown native 밑줄 값을 None으로 만들지 않고 owner의 원문 wrapper와 metadata를 그대로 보존한다.

입력 언어는 authored의 closed creation profile 또는 native의 명시 source language/LTR를 보존하는 **선언된 앱 정책**이다. 실제 OS InputLanguageManager/CurrentInputLanguage와 같다고 주장하지 않는다. IME·Paste·Cut·일반 드래그 선택·임의 full-range 대입은 이 ordinary Text 권한을 얻지 않는다. idle에서는 캐시된 source/profile을 재사용하며, 이번 단계에서 새 성능 개선 측정을 주장하지 않는다.

native 저장은 같은 평문이라도 Run body 분포가 바뀔 수 있음을 구분한다. original root/Paragraph와 모든 Run opening·closing·수·순서가 exact인 raw-body-only 변경은 ordinary content COW로 처리한다. 이전 서식/분할 때문에 이 증명이 없으면 기존 format-only 증명이 필요하다. 둘 다 실패하는 same-original-plain 대체는 UI 게시 전에 원자적으로 거절한다. 다른 평문으로 바꾸는 기존 content 경로는 그대로다. 증명 실패를 content로 전환하는 fallback은 추가하지 않았다.

authored의 실제 full selected Backspace는 삭제 전에 정규화된 첫 nonempty Run의 current format을 캡처한다. 전체 원문에서 모든 body만 비운 결과와 candidate 전체 XAML·seed·서식을 대조한 뒤 제한된 후속 ordinary Text owner를 게시한다. 두 개 이상의 nonempty Run이 걸친 새 경로와 기존 unique-Run 경로를 구분한다. Delete/Cut·빈 wire·IME에서 같은 권한을 만들지 않는다. 빈 문단의 시각적 캐럿은 Paragraph glyph 정책을 사용하므로, ordinary typing preview 전에는 캡처한 Run의 크기·서식과 다를 수 있다. 기존 Backspace affinity의 focus/lifetime 정책을 이번 단계가 전부 해결했다고 주장하지 않는다.

남은 범위는 임의 선택의 비어 있지 않은 치환, 여러 문단·병합·LineBreak·newline, Paste/IME 대체의 원본 계약, 일반 caret/springload, 실제 OS 입력 언어 전환, 원본 serializer의 Run 정리/병합 및 wrapper byte 동형성이다. 그림·임베디드·전체 Text Block reflow와 r214의 nonLeft Enter/일반 mapped alignment 제한도 그대로다.

기능 매핑은 D08과 HD035/HD040의 **부분 구현 깊이**를 갱신한다. HD051의 기존 단일 LTR 정렬은 새 상태 전이가 아니다. HD015는 worksheet의 모든 영역 선택이므로 focused Text의 Ctrl+A와 혼동하지 않고 미구현으로 유지한다. HD018 영역 삭제·HD052 검색/치환도 별개다. 세부555행(부분157/미구현398), 상위87행(구현4/부분69/미구현14)은 유지한다.

원본 근거는 [SelectAll 정적 CIL 계약](../../artifacts/rea/rich-text-selectall-next-20261020.md), [삭제 경계의 정적 범위](../../artifacts/rea/rich-text-boundary-deletion-next-20261020/README.md), [selected Backspace capture/reapply](../../artifacts/rea/rich-text-postdelete-next-20261020.md), [전체 선택 후 삭제 조사 ledger](../../artifacts/rea/rich-text-after-full-delete-next-20261020/ledger.json)를 구분한다. [native 원문 owner/저장 proof 검토](../../artifacts/rea/native-selectall-raw-owner-next-20261020/review.md)는 unknown 속성의 보존과 export 경계를 설명하며 실제 원본 runtime 관찰이 아니다.

## 사전 오류 재현과 수리

native의 partial Run 서식 분할 뒤 Ctrl+A로 **같은 원본 평문**을 입력하면, 저장 증명을 충족하지 못하는 새 XAML을 먼저 게시하는 실제 제품 오류를 [정확한 두 회귀의 기존 제품 실행](../../artifacts/rea/select-all-r216-prior-split-old-product-exact-filter-preflight-20261020.log)에서 재현했다. 다른 평문 양성은 통과하고 same-plain 원자성 검사가 실패했다. [분리한 제품 guard](../../artifacts/rea/native-selectall-sameplain-admission-r216-next-20261020/product-guard-only.patch)를 적용한 뒤 [변경하지 않은 회귀와 기존 native 사례](../../artifacts/rea/select-all-r216-native-publication-guard-preflight-20261020.log)가 통과했다. 이 로컬 수리는 아래 양 OS 고정 검증과 별개 기록이다.

[먼저 사용한 잘못된 이름 필터](../../artifacts/rea/select-all-r216-prior-split-old-product-preflight-20261020.log)는 0건을 실행했으므로 통과 근거로 쓰지 않는다. [C의 첫 full Backspace 앱 컴파일](../../artifacts/rea/select-all-r215-full-backspace-first-preflight-20261020.log)은 테스트의 private wire 필드 접근·범위 밖 Scratch helper 오류였으며 [fixture 접근 수리 후 검사](../../artifacts/rea/select-all-r215-full-backspace-test-repair-preflight-20261020.log)와 구분한다. A의 [첫 sibling method visibility 컴파일 오류](../../artifacts/rea/select-all-r215-app-first-preflight-20261020.log)도 [worksheet-scoped 가시성 보정 후 검사](../../artifacts/rea/select-all-r215-app-visibility-repair-preflight-20261020.log)와 구분한다. 이 기록을 원본 동작 검증이나 새 제품 계산 오류로 합쳐 세지 않는다.

r215 사전 source archive는 미채택 이력으로 보존하며 양 OS driver를 시작한 실패로 서술하지 않는다. r216의 실제 archive·manifest·accepted receipt와 재시도 전체 목록은 아래 고정 검증과 채택 receipt에 기록했다. 과거 finalizer를 다시 실행하지 않는다.

## 고정 검증

| 항목 | 결과 |
|---|---|
| Windows 정상 검사 | 1975 |
| peets 정상 검사 | 1976 |
| ignored Windows / peets | 100 / 98 |
| MCDX / 별도 XML / XSD parts | 192 / 10 / 1162 |
| PDF | 120 |
| source archive SHA | `15bbd488825f5d4ec1cb21ff74d49d557d9363a56eb27d1fa22d01155054bcd3` |
| source manifest SHA | `140acaf4bf79cf092bd6e964276f8cc07d5f5dece51dce11ff1124a20bb32442` |
| accepted validation SHA | `542442b3176867da5d9efb1f23bc566166361e3f056dab8d3187592e9b6f90af` |

새 독립 선택 편집 PDF packet은 없다. 일반 회귀의 임시 native/own 저장·cold 검사를 독립 packet/PDF 상태 수에 더하지 않는다. 실제 인벤토리와 전체 prior failures/retries는 [채택 receipt](../../artifacts/rea/select-all-integration-20261020-r216/validation.json)를 따른다. GUI 지연·GPU·원본 재열기·글꼴/픽셀 동등성은 미검증이다.
