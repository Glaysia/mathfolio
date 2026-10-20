# A03: 절대온도·온도차의 원본 계약

정적 근거와 다음 구현 범위를 정리한다. 제품 구현이나 원본 런타임 비교의 완료 판정이 아니다. GUI·스크린샷·CLR/COM·원본 계산 엔진·Cargo는 실행하지 않았다.

원본 source·상수·함수 inverse 쌍을 확인했다. 상세 본문은 [정적 계약](../../artifacts/rea/affine-temperature-next-20261019.md), 설치 SHA/MVID와 12개 메서드 바이트 검증은 [ledger](../../artifacts/rea/affine-temperature-next-20261019.json), 13개 설치 Help MathML 원문은 [fixture 인덱스](../../artifacts/rea/affine-temperature-next-20261019/help-and-fixtures.json)에 있다.

## 확정된 원본 동작

- °C/°F는 Kelvin으로 변환하는 **함수**다. 결과의 units placeholder에서는 반대 변환을 수행한다. Δ°C/Δ°F는 별도의 선형 온도차 단위다.
- 설치 Help는 `<apply><scale/><real>20</real><id labels="UNIT">°C</id></apply>`와 `<unitOverride><id labels="UNIT">°F</id></unitOverride>`를 실제 textarea MathML로 제공한다. source에 새로운 affine 전용 XML 요소를 발명할 필요가 없다. 일반 `°C(x)` 구문도 도움말에 명시되어 있다.
- mcdunitsystemsi.dll의 원본 CIL이 Coffset=273.15K, Ffactor=(5/9)K, Foffset=Coffset−32·Ffactor와 forward/inverse 함수를 구성한다. 함수는 일반 곱셈·덧셈·뺄셈·나눗셈을 사용하며, 원본 Mpl.Function inverse 포인터로 서로 연결된다.
- Δ°C는1K, Δ°F는(5/9)K다. unit selector의 degC/degF는 IsFunction=True이며 deltaC/deltaF와 구별된다. degree/angle 기능과 혼동하지 않는다.
- 원본 Help의 `24°C + 6°C` 설명은 물리적으로 의도하지 않은 결과와 `6Δ°C`의 필요성을 보여준다. 이 근거로 절대온도끼리의 덧셈에 새 TypeError를 도입해서는 안 된다.

## 제안하는 첫 구현 범위

유한 실수 scalar의 dimensionless→Kelvin forward와 temperature Quantity→dimensionless 표시 계수 inverse를 우선한다. 기존 Quantity·차원 연산을 유지하면서 affine builtin 함수와 inverse 경로를 별도로 표현한다. °C를 단순1K 곱셈 단위로 등록하거나 모든 곱셈에 offset을 더하는 구현은 피한다. UNIT label, 함수 source, unitOverride, canonical 물리량과 표시 계수는 분리해 보존한다.

원본 source fixture를 이용한 향후 검증은 0°C/20°C/32°F/212°F, °C→°F 표시, Δ 단위 합·비율, label·사용자 shadowing·차원 오류, 저장/재개방 시0계산을 포함해야 한다. 예상 수치는 원본 상수에서 도출한 테스트 값으로 기록하고, 캡처된 원본 실행 결과라고 부르지 않는다.

## 미확정 경계

array/complex/vectorization, 정확한 native 오류 ID·우선순위, generic 사용자 단위함수 inverse 추론, XY affine scaling, 타 단위계, absolute-zero 제한은 이번 근거로 완성됐다고 하지 않는다. 특히 affine unitOverride 적용 후 original cached result와 unitMonomial의 정확한 carrier는 아직 확인하지 못했다. Kelvin 값에 degC 메타데이터를 임의로 붙여 새 native cache를 쓰지 않는다. 기존 원본 캐시는 보존하고, 새 native 출력은 별도 근거가 생길 때까지 지원 경계를 명시한다.

현재 단계는 구현 준비다. 제품 소스 변경이나 테스트 실행은 이 조사에서 하지 않았다.
