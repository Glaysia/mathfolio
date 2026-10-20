# 구조 연산자의 앱·저장·입력 백엔드 통합

2026-10-16. [원본 정적 계약](0069-structural-array-operators-contract.md), [입력·조판](0070-structural-array-operators-input.md), [계산](0071-structural-array-operators-calculation.md)을 리본·문서 저장·표시 캐시까지 연결했다. GUI·스크린샷·원본 앱·COM·CLR을 실행하지 않았다.

리본의 전용 명령은 factorial·transpose·conjugate·절댓값/행렬식·Norm·행 추출·열 추출이다. 기존 함수 호출 갤러리와 구분한다. 원본 BAML/descriptor/factory대로 절댓값/행렬식은 `absval` 단선, Norm은 `determinant` 이중선이며 새 `<norm/>`를 만들지 않는다. 모든 명령은 현재 피연산자를 유지하고 한 영역의 Undo로 복원하며 계산은 초안/확정 경계를 따른다. 행렬 셀 직사각형 typeover에서 거절하는 명령은 문서와 이력을 바꾸지 않는다.

표시 캐시는 Overline, 단선/이중선 Bars, 행/열 방향 AngleFence를 구조로 저장한다. 이름 suffix나 일반 power의 의미로 바꾸지 않는다. 저장된 symbolic MathML의 새 연산자와 복소수 표시도 계산 없이 복원하며, 크기·깊이·노드 예산을 유지한다. 같은 precedence라도 원본이 금지한 row/column/transpose 중첩은 괄호로 구분한다. source AST를 바꾸지 않는 읽기 전용 표시와 실제 편집기의 grouping 삽입을 별도로 검사한다.

## 키 이벤트를 보존하는 백엔드 수정

설치된 OSS `egui-winit 0.36.2`는 C 키의 Copy 판정에서 Shift/Alt를 무시하고 원래 Key를 버렸다. 이를 Copy+modifier로 추측하면 Windows Ctrl+Shift+Insert나 전용 Copy 키·메뉴 Copy도 열 연산자로 오인한다. 따라서 그런 추측을 제거했다. `vendor/egui-winit`의 원래 C 분기에 `!shift && !alt`를 추가해 Ctrl+Shift+C가 Key로 남게 한다. 원본 레지스트리 archive/source 해시, 변경 기록과 upstream 라이선스는 같은 폴더에 보존했다. 다른 source 파일 4개는 바이트 동일하다. Rust 실행 파일의 별도 라이브러리 수나 Mathcad 리소스를 늘리는 작업이 아니다.

`egui-winit`의 CPU 단위 검사 2개와 앱의 실제 global history/clipboard→page 경로 검사를 분리한다. 정확한 Ctrl+Shift+C는 Column, 실제 Copy 이벤트는 modifier와 관계없이 복사이며 clipboard를 잘못 덮어쓰지 않는다. `scripts/check.ps1`/`check.sh`에 이 백엔드 검사도 넣었다. 실제 데스크톱 키 입력·클립보드 실기 검사는 이 증거에 포함하지 않는다.

## 파일·출력 경계와 남은 범위

ignored `structural_operators_save_native_export_reopen_probe`는 위 리본 명령 7개로 수식을 만든다. factorial 120, 비켤레 전치 3×2, 켤레 2−3i, 두 세로선 명령의 정방 determinant −2, 행 추출 1×3과 열 추출 2×1을 비교한다. 자체 저장 재열기는 표시 7개·계산 0회·Pending 없음, MCDX import 후에는 모든 입력이 Math이며 명시 재계산 오류 0개를 확인한다. 재가져온 native 문서도 결과 캐시와 함께 별도 저장해 벡터 PDF를 검사한다.

실제 1015/1016 표·기존 인덱스 정의·행렬 범위 저장 회귀와 함께 Windows/peets의 동일 소스를 검사하며 최종 수·해시·로그는 [CONFORMANCE](../CONFORMANCE.md)에 둔다. 전체 원본 편집/선택·기호 변환·행/열 좌변 대입·nested 값·모든 단위 타입 lowering·원본 글꼴 및 픽셀 배치는 미완료다. 이 작업과 테스트 수를 전체 완성도 20% 달성으로 환산하지 않는다.
