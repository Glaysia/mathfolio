# 결과 서식 리본·표시·저장·인쇄 통합

2026-10-16. 원본 [명령·직렬화 계약](0045-result-format-contract.md)과 [문서 모델](0046-result-format-persistence.md)을 적용한다. [제공 PDF 비교](0043-pdf-output-comparison.md)에서 확인한 백분율·극형식 손실은 서식 기능을 연결하지 않은 결함이었다. PDF의 표시 숫자를 문자열 치환해 고치지 않는다.

## 연결한 경로

원본 결과 그룹에 대응하는 리본에 General/Decimal/Scientific/Engineering/Percent, 소수 자릿수, 후행 0, 복소수 i/j 및 극형식 rad·π rad·도, 영·복소수·지수 임계값을 연결한다. 지수 임계값 UI는 General에서만 활성화하며 UI 상한 15와 파일 상한 17을 구분한다. 근거가 확인되지 않은 E-notation 변경 버튼은 추가하지 않고 파일의 설정만 읽고 보존한다.

숫자 서식은 계산의 실수·허수·차원을 바꾸지 않는다. Percent는 표시할 때 100배와 %를 적용하며 입력의 % 연산자를 새로 구현한 것이 아니다. Polar는 복소수 결과 표시이며 입력 Polar 편집 노드의 구현이 아니다. 후행 0·절대 영 임계값·상대 복소수 성분 임계값·General의 지수 전환·과학/공학 및 저장된 E 표기를 동일한 수치 표시 경로에 적용한다. live 결과, native의 inert literal, 행렬 원소가 이 경로를 공유한다.

MCDX 가져오기는 문서 presentation의 기본값을 먼저 읽고 영역별 존재하는 속성만 덮어쓴다. 저장된 `polarDegrees`를 버리고 직교형식으로 표시하던 결함을 고친다. 서식 변경 명령은 Undo·복사·자체 저장과 MCDX export에 연결하며 수식의 MathML 실행을 요청하지 않는다. 원본의 전체 문서 서식 적용/다중 선택/속성별 모든 상속 변경 UI까지 완성한 것은 아니다.

## 저장 결과의 정밀도와 재계산

반올림한 표시 문자열만 저장하면 0.944를 다시 94.359%로 표시할 수 없다. 그래서 지원되는 결과는 표시 tree와 함께 원래 정밀도의 inert source를 보존한다. native 결과는 literal MathML, 독립 계산 결과는 Value JSON이며 계산 바인딩·함수 환경·실행 프로그램을 저장하지 않는다. source는 크기·원소 수·행렬 shape·유한 수/차원을 검사한 후 표시만 한다. 잘못된 데이터로 수식을 실행하지 않는다.

서식 cache가 달라지면 이 source로 표시를 다시 만든다. native의 선택 단위가 이미 적용된 숫자는 추가로 단위 환산하지 않고 기존 단위 glyph 중복도 피한다. 자체 문서를 저장하고 계산기를 초기화해 다시 연 뒤에도 지원 원시 결과가 있으면 서식을 바꿀 수 있다. 이전 버전의 표시 문자열만 있는 캐시는 원래 정밀도를 복구할 수 없으므로 Pending과 계산 필요 상태를 알리고 숫자를 추정하지 않는다.

인쇄 snapshot도 같은 표시 cache를 사용한다. Percent·극형식·자릿수를 인쇄용 별도 규칙으로 바꾸거나 인쇄 때문에 재계산하지 않는다. 별도 인쇄 경로가 편집 placeholder를 그리던 문제는 [0043](0043-pdf-output-comparison.md)의 readonly 수정으로 분리했다.

## 검사와 남은 경계

회귀는 0.94359의 94.359% 표시·계산값 불변, precision을 잃지 않는 cold reopen과 서식 재변경, 문서 기본값과 local 극형식 override, Undo·MCDX export→import→명시 재계산, 잘못된 numeric cache 거부, 인쇄 snapshot의 동일 표현을 검사한다. 원본 리소스·코드·글꼴을 새 제품에 복제하지 않고 GUI·원본 런타임 없이 검사한다. 최종 두 OS 결과는 [CONFORMANCE](../CONFORMANCE.md)를 따른다.

기수·분수 숫자 표시·round-trip precision sentinel의 실제 표시, 전체 문서 기본값 변경 UI, 다중 선택과 Clear Format은 미완료다. 원본 두 formatter의 0%·경계 반올림 차이와 일부 pure-real 복소수 타입 표시는 모든 소비자 경로를 확인하지 못했다. `use-engineering-format`은 XSD 설정을 보존하지만 원본 General formatter 근거 없이 새 동작으로 해석하지 않는다. General 외 임계값의 독립 보존 결정은 [0046](0046-result-format-persistence.md)에 구분한다.

제공된 두 PDF는 변경하지 않았다. 표·rich text·색상/강조·전체 수식 조판·선택 가능한 벡터 PDF는 이 변경으로 해결됐다고 주장하지 않는다.
