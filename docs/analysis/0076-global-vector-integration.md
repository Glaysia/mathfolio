# 전역 정의·벡터화·외적의 앱 통합

2026-10-16. [원본 저작 계약](0073-global-vector-operator-contract.md), [키·조판](0074-global-vector-operator-input.md), [계산](0075-global-vector-operator-calculation.md)을 리본·초안 확정·계산 대상·저장 캐시에 연결했다. GUI·스크린샷·원본 런타임·COM·CLR을 실행하지 않았다.

전역 정의는 별도 GlobalDefine root로 저장하며 화면 위쪽 수식에도 적용한다. 입력 중에는 계산하지 않고 Enter에서 확정한다. F5의 화면 prefix 계산도 전체 문서의 전역 선계산을 유지한다. 잘못된 전역 초안을 오류 statement로 바꿀 때 일반 define으로 낮추던 경로를 교정해 이전 성공값이 남지 않게 한다. 관련 없는 식의 계산은 계속한다.

Vectorize는 수식 안 연산자와 함수 호출을 원소별로 계산한다. 일반 행렬 곱과 벡터화한 곱을 구분하고 scalar를 broadcast한다. CrossProduct는 3×1 벡터의 비켤레 외적이며 원본의 오른쪽 결합·⨯ glyph를 따른다. 새로운 전역·벡터 리본 명령도 행렬 범위 typeover를 덮어쓰지 않는다.

SpecTable의 header/unit/body는 원본의 AllowExpressionsOnly=true를 따르는 Editor context다. 정의와 전역 정의는 키·리본·clipboard replacement를 통해 삽입할 수 없다. 거절된 명령은 arena revision, 초안, 문서 dirty, 계산 요청, Undo를 만들지 않는다. context는 문서 내용이 아니므로 Undo가 되돌리지 않으며 clone에서 유지한다. 일반 식 입력·셀 확정은 유지한다.

파일 probe는 10영역에서 전역 gain=3, 일반 곱=32, 벡터화 곱=[4,10,18], 외적=[−3,6,−3], 사용자 함수의 벡터화=[3,6,9], scalar broadcast=[4,5,6]을 검사한다. 자체 저장 재열기는 표시 6개·계산 실행 0회·Pending 없음, native MCDX 재계산은 오류 0개이며 모든 입력을 Math로 가져온다. 재가져온 문서의 캐시를 별도 저장해 벡터 PDF를 검사한다.

동일 소스의 Windows/peets 검사 결과·실행 파일 해시·실제 1015/1016 표 회귀·XSD/PDF 결과는 [CONFORMANCE](../CONFORMANCE.md)에 기록한다. 전체 원본 타입·nested 배열·미적분/상징 벡터화·행렬 이름 전역 좌변·모든 편집 전이·원본 글꼴/픽셀 배치와 실제 데스크톱 사용성은 미완료다. 이 묶음과 테스트 수를 전체 Mathcad 완성률 20%로 환산하지 않는다.
