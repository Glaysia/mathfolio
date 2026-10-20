# 행·열 좌변 정의와 문서 확정 경계

2026-10-19. [원본 저작 계약](0081-array-slice-definition-contract.md)과 [계산 계약](0082-array-slice-definition-calculation.md)을 worksheet의 일반 정의·전역 정의·프로그램 내부 대입에 연결했다. GUI·스크린샷·원본 실행 비교 없이 설치 파일의 REA 증거와 Mathcraft CPU 검사로 확인한다. 전체 제품 완성률을 검사 개수로 환산하지 않는다.

원본 native 표현인 `apply(matrow|matcol, named-base, scalar-index)`를 그대로 정의의 왼쪽에 보존한다. `ORIGIN`, structured identifier의 label/contextual/literal suffix, 전역 정의의 앞쪽 참조, 프로그램의 지역 scope를 유지한다. 범위 값의 slice index는 원본 frontend lowering 확인이 부족해 명시 오류로 제한한다. 일반 indexed 정의의 기존 range 지원은 유지한다. Row의 일반 matrix RHS는 저수준 setter의 직접 증거이며 상위 typing 전체의 일치로 확대하지 않는다.

## 확정·실패·이력

행·열 추출 식에서 정의를 삽입하고 RHS를 바꾸는 동안 계산 결과는 직전 확정 상태를 유지한다. Enter로 확정하면 해당 base 이름을 갱신하고 뒤쪽 질의를 계산한다. 잘못된 초안의 portable tree나 raw MathML에서 계산 실패용 statement를 만들 때에도 `matrow`/`matcol`의 base를 찾는다. 전역 실패는 `globalDefine`을 유지한다. 이 처리가 없으면 직전의 성공한 행렬 binding이 남아 새 대입이 실패했는데도 이전 값을 보여 줄 수 있었다.

같은 경로의 리본 Define·Evaluate·InsertSymbol은 수정한 math region의 이력만 보존한다. 기존 분기는 매번 문서 전체를 복사해 이력에 넣고, Undo 시 다른 live editor도 재생성했다. 구조 수식·서식 명령에서 사용하던 region 이력과 통일했으며 다른 수식의 입력 중 초안을 Undo/Redo로 덮어쓰지 않는 회귀를 추가했다. 새 영역을 만드는 명령은 기존 전체 문서 이력을 유지한다. 이 복사 제거 자체의 속도는 별도로 수치화하지 않았다.

앱의 새 일반 검사 네 개는 다음을 확인한다.

- 행·열 정의의 초안·Enter·계산 결과·두 단계 Undo/Redo와 region 이력.
- 잘못된 Row/Column 일반·전역 정의가 이전 binding을 드러내지 않으며 독립 수식은 계산되는지.
- raw slice의 base 이름·explicit label·contextual flag·전역 태그의 실패 전달.
- Define·Evaluate·InsertSymbol 각각의 Undo/Redo가 다른 live editor의 초안을 보존하는지.

## 저장·native 왕복 probe

`array_slice_save_native_export_reopen_probe`는 13개 영역을 만든다. 열 대입 후 행 대입, 변경 전 alias, 아래쪽 전역 정의를 참조하는 위쪽 질의, 프로그램 내부 지역 열·행 대입, ORIGIN=1 이후 대입을 포함한다. 여섯 질의의 행렬 원소를 독립 기대값과 비교한다. 원본 태그로 MCDX를 저장하고 전 영역이 편집 가능한 Math로 재수입되는지, 재계산 오류가 없는지 확인한다. 자체 저장 문서는 계산 없이 여섯 표시 cache를 복원해야 한다.

이 probe는 자체 native exporter/importer의 왕복 검사이며 Mathcad를 실제 실행해 읽힌다고 확인한 것은 아니다. 설치 XSD와 PDF 객체·텍스트 검증 및 양쪽 OS의 소스 동일성은 [CONFORMANCE](../CONFORMANCE.md)의 최신 통합 검증에 별도로 기록한다. 입력 백엔드의 Ctrl+Shift+C 보존과 Ctrl+- literal 아래첨자 회귀도 전체 검사에 계속 포함한다.

확정 준비 비용 변경과 cold 한계는 [0084](0084-prepared-source-cost.md)에 기록한다. 원본의 전체 편집기·프로그램 저작·상징 계산·타입 조합·viewport 조작은 여전히 부분 구현이다.
