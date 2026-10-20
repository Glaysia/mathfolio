# 백분율·도·극형식 입력의 앱 통합

구현 2026-10-16, 통합 검증 2026-10-19. [원본 저작 계약](0077-scalar-notation-contract.md)과 [계산 계약](0078-scalar-notation-calculation.md)을 직접 수식 편집·리본·확정·캐시·native 저장에 연결했다. GUI·스크린샷·원본 앱·COM·CLR을 사용하지 않았다.

`%`와 `°`는 후위 노드이며 결과 Percent 서식과 별개다. Ctrl+Shift+P는 Polar 연산자를 만든다. 일반 Ctrl+P 인쇄 명령이 이를 가로채지 않도록 실제 global→page 입력 경로를 검사했다. 이름 아래첨자를 편집할 때 `%`·`°`는 suffix 문자이며 수학 연산자로 변환하지 않는다. 선택한 일반 행렬 범위를 세 명령으로 덮어쓰지 않는다. 표의 expression-only 문맥에서는 정의가 아닌 일반 수식으로 사용한다.

원본 `VisualFactory.CreateInlineUnaryNode` token `0x06000683`, IL SHA `d3a0064f5b475b5315b3eda6714920c0e96acd26ba29b8d5eaed66e49339179b`는 VisualSymbolNode와 VisualLinearUnaryNode를 만든다. 해당 node의 `.ctor` `0x060008bd`, `MeasureStaticLayout` `0x060008cc`, `ArrangeStaticLayout` `0x060008cd`를 원본 artifact SHA 및 기존 REA metadata와 대조해 정적으로 읽었다. suffix는 child 다음 symbol을 배치하며 폭을 더하고 baseline을 맞춘다. 독립 렌더러도 이 순서를 따른다. Percent의 Power/NthRoot 강제 그룹 규칙을 Degree에 복사하지 않는다. 원본 glyph 윤곽·모든 script baseline·화면 픽셀 일치는 미검증이다.

원본 파일은 `Ptc.Controls.EquationControl.dll`, SHA `0b6fd580eb6cc183f17b5b5e0967226bbafc5e12efc6da809f44cd16416bf9d1`, MVID `cf1d7cb5-c73f-4618-8e6b-b83959d13b98`다. 새 선택 REA evidence는 `ev_39003d2a52e853bd51294f2ca953a73a3c1c7222d8fbd03834db94e3a14a8b98`, method page는 `ev_764f4f83e67426551a3cf4bef7f0cb4871c9ac4bfaef0219d3b060cf0dfe5a75`다. 보충 raw CIL의 해시·범위는 ignored `artifacts/rea/scalar-notation-visual-{decode.py,cil.json,cil.txt}`에 보존했다. 정적 파일 해석이며 원본을 실행한 결과가 아니다.

Polar는 왼쪽 결합, CrossProduct는 오른쪽 결합이다. 독립 선형 입력은 `a∠b⨯c`를 `Polar(a,Cross(b,c))`, `a⨯b∠c`를 `Polar(Cross(a,b),c)`로 만든다. raw 트리의 표시 괄호는 이 결합 차이를 유지하고 source AST를 바꾸지 않는다. 원본 descriptor 교체 시 group repair는 별도 경로라 모든 신규 입력·raw renderer와 동일하다고 주장하지 않는다. 자세한 정적 한계는 0077에 기록했다.

파일 probe는 `50%=0.5`, `90°=π/2`, `2∠90°=2i`, `200·10%=20`, `sin(90°)=1`, `(2∠90°)·V=2i V`, 2×2 행렬의 Percent를 검사한다. Polar 내부에 물리 단위를 허용하는 새로운 overload를 만들지 않으며 완성된 phasor에 단위를 붙인다. 자체 저장 재열기는 표시 7개·계산 실행 0회·Pending 없음이고 native MCDX 재계산 오류는 0개다. native 결과 캐시의 %·°·∠ 표현도 계산 없이 구조로 복원한다.

[확정 준비 비용 조사](0079-commit-preparation-cost.md)는 같은 단계의 별도 성능 변경이다. 동일 소스의 Windows/peets 검사·실제 문서 회귀·XSD/PDF·실행 파일 해시는 [CONFORMANCE](../CONFORMANCE.md)를 따른다. 전체 원본 편집 전이·nested 값·상징 변환·DMS 함수와 실제 OS 입력/렌더 사용성은 미완료이며 이 작업을 전체 완성률 20%로 환산하지 않는다.
