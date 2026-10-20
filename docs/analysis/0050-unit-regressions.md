# 재계산 뒤 단위 표시가 바뀌는 문제

2026-10-16. 사용자의 단위 표시 오류를 실제 세 워크시트로 재현했다. 원본 Mathcad 실행·CLR·COM·GUI·스크린샷을 사용하지 않았다. 저장된 원본 결과는 역사적 비교 자료이며 새 원본 엔진 실행의 정답 oracle이라고 주장하지 않는다.

## 재현과 원인

저장된 원본 결과에는 V·Ω·W처럼 단위 시스템이 고른 이름이 있지만, 명시 재계산 뒤 `result_display::quantity`는 물리 차원을 항상 m·kg·s·A 등의 기본 단위로 풀어 그렸다. 예를 들어 detune의 원본 영역 13, 위치 `(377.95275590551176, 188.97637795275588)`은 같은 숫자 `277.489 V`가 재계산 뒤 `277.489 m²·kg/(s³·A)` 형태로 변했다. 원본 영역 14의 Ω, 영역 75의 W도 같은 종류의 문제다. 실제 Quantity의 숫자·차원이 손상된 것은 아니지만 결과 단위의 모양과 폭이 바뀌어 수식을 읽기 어렵게 했다.

검사는 앱과 같은 editable bridge 경로로 원본 MathML을 PortableTree로 읽고, `worksheet_xml`의 계산용 contextual label 해결을 거쳐 독립 엔진에서 전체 계산했다. 직접 raw `native-run`을 실행하면 원본의 contextual label을 해결하지 않아 다수의 오류가 생기므로 그 결과를 앱의 단위 오류로 오인하지 않았다. 검사 도구는 원본 워크시트를 수정하지 않는다.

| 원본 파일 | SHA-256 | 계산 행 | 숫자가 일치한 저장 결과 | 단위 이름 불일치 전 → 후 |
| --- | --- | ---: | ---: | ---: |
| `Downloads/detune.mcdx` | `bd4b329ce3cb2de848843c80e3bd42570022dc54a2c7d335f898c234151c1cbf` | 66 | 50 | 23 → 0 |
| `Desktop/0818.mcdx` | `d2ab0e529b67c76ad580cbca446502b5041449beb3929cd54886d4e53cf04cba` | 56 | 45 | 18 → 0 |
| `Desktop/1kW_LCCL-S 설계.mcdx` | `55248ce55262e7a22e4c2dcbe858c6d97ca82892d75f498a2c47432ac44e846f` | 50 | 39 | 15 → 0 |

불일치 56곳의 구성은 detune V 10/Ω 12/W 1, 0818 V 7/Ω 8/W 3, 1kW V 6/Ω 6/W 3이다. 수정 전후 172개 행 모두 계산 오류가 없고 134개 비교 가능한 저장 숫자가 허용 오차 안에서 일치했다. 엔진의 `value`·`display_value`, 입력 MathML·단위 slot은 수정 전후 그대로다. 원본 영역 ID·원래 left/top·saved/current 단위·입력 식을 포함한 전체 목록은 ignored `artifacts/rea/unit-regression-comparison.json`에 있다.

추가로 사용자가 지정한 실제 원본 `Y:/26_q4/CKT/1015/1015_LCC_S.mcdx`와 같은 폴더의 `1016_LCC_S.mcdx`도 검사했다. Rust의 파일 시스템 조회가 Y:에서 OS error 1005를 반환하여 PowerShell로 ignored 로컬 조사 폴더에 복사했다. 아래 SHA는 읽은 복사본의 스냅샷 정체성이다. 1015는 원본 경로의 SHA와도 일치함을 확인했다. 1016 원본의 후속 SHA 재조회는 다른 프로세스가 사용 중이라 거절되어, 해당 시점 원본과의 재대조를 완료했다고 주장하지 않는다. 수정한 renderer로 각각 워크시트 최상위 수식 79개를 계산했으며 오류가 없고 각 64개 저장 결과 숫자와 자동 결과 단위 이름이 일치했다. 이 두 파일은 수정 뒤에 추가 제공되었으므로 이전 renderer와의 before count를 날조하지 않는다.

| 추가 원본 | SHA-256 | 계산/저장 숫자 비교 |
| --- | --- | --- |
| `1015_LCC_S.mcdx` | `217994a631de58e096a3cfdb8a8251aca91501e62151f869950b99f738fc18b5` | 79행 오류 0, 64개 숫자 일치, 자동 단위 이름 불일치 0 |
| `1016_LCC_S.mcdx` | `9385cd1c4f2ce7a0e23d7474560040f6fde886e06c8071e634350a7866005932` | 79행 오류 0, 64개 숫자 일치, 자동 단위 이름 불일치 0 |

두 파일 모두 원본 영역 26의 `<spec-table>` 안에 여섯 `math` 정의가 별도로 있다(전체 math 85, 최상위 math 79). 이 표 정의는 이번 계산 probe에 포함하지 않았지만 최상위 79행은 전부 정상 계산되었다. 표 밖 수식의 성공을 표 내부의 binding·matrix·단위 header 실행 계약까지 구현됐다는 근거로 확대하지 않는다. 표의 구조·표시·계산 의미는 별도의 구현 범위다.

후속 [0054](0054-spec-table-calculation.md)에서 표를 별도 구현·검증했다. 같은 두 스냅샷에 표 region을 포함한 80개 외부 행 오류 0, 실제 여섯 열을 참조하는 downstream query 오류 0, 숫자 55개와 물리 차원·단위 scale 일치를 확인했다. trailing 빈칸 정규화의 원본 근거 범위와 아직 직접 확인하지 못한 native lowering은 해당 문서에 명시했다.

## 단위 선택의 정적 근거

[0010](0010-unit-semantics.md)의 인증된 SI 상수 추출기를 재사용했다. 기존 추출기는 `unit`·`unit_system`·`unit_system_group` 호출을 실행하지 않고 metadata marker로 남겼다. 이번 ignored `derive-si-display-facts.py`는 그 marker에 inert constructor arguments와 원본 IL 위치만 보존하여 system 목록을 읽는다. 원본 함수를 CLR로 호출하거나 수치 함수의 해석 범위를 넓히지 않는다.

- `mcdstaticunitsystem.dll`: SHA `5ba19b272cbbeb27c3e171a4bb83d1d14ae8c518cc9271810276d5eee699cb9b`, REA `ev_2ef822c8cdf3873a1c829ef26a26aa9c2e13f4c5c6aae03c13c90bbfd0e7f7dc`.
- `mcdstaticunitsystemsi.dll`: SHA `8a3665f49f25c8ca83c614ec5c934d194fab2685fe396b86517d56791a0d2a01`, REA `ev_516f379510019ca27af078efe7f8a95c4f78cd4a34dfebaa31d3080a28ab592c`.
- SI `values.Update`, token `0x06000001`, IL SHA `ef8954ea5b78a57ce90e2dcb5f0a067af9278398e8b274d3cbfe523b7a50bd32`가 system field `0x04000017`을 IL `0x064b`에서 저장한다.
- `mcdrun.dll`: SHA `7a1f2d6926cde4af49f45f814d2ae01655562ebb1264a9af39a742edc3326009`, REA `ev_bd54b6d6912bc5b9065343a3e385ae3aad30e8cfe83786f472537f23b1de7042`. `unit_system` token `0x06000016`, IL SHA `f55f717399e5d20a9d157ecb3ff62fb6e762741aa1372ba1fdee80cb01b198e6`는 metadata array의 두 unit list를 `Mcd.UnitSystem` 생성자에 전달한다.

SI metadata의 이름·기호·수치·차원 기록에는 기본 8개 차원 단위와 다음 named derived candidates 14개가 있다. Money 차원은 기존 Quantity가 지원하지 않아 이번 변경으로 추가하지 않았다.

| 이름/기호 | SI Update의 unit constructor IL 위치 |
| --- | --- |
| coulomb / C | `0x035d` |
| farad / F | `0x0390` |
| henry / H | `0x03c3` |
| joule / J | `0x03f6` |
| liter / L | `0x0429` |
| lux / lx | `0x045c` |
| newton / N | `0x048f` |
| ohm / Ω | `0x04c2` |
| pascal / Pa | `0x04f5` |
| siemens / S | `0x0528` |
| tesla / T | `0x055b` |
| watt / W | `0x058e` |
| weber / Wb | `0x05c1` |
| volt / V | `0x05f4` |

이 기록과 위 실제 결과의 V·Ω·W 선택이 구현의 근거다. 전체 원본 unit decomposition 알고리즘을 복원했다는 의미는 아니다. 임의로 catalog의 첫 차원 일치 항목을 고르지 않는다. 예를 들어 Hz는 이 named derived candidate 목록에 없으므로 자동 결과를 추측으로 Hz로 바꾸지 않는다.

## 수정한 경계

표시하려는 Quantity의 차원이 위 대표 SI 단위 하나와 정확히 맞으면 해당 이름으로 그린다. L은 원본의 0.001 m³ 배율로 표시 숫자를 바꾸고, 나머지는 원본 scale 1이다. 일치하지 않는 복합·분수 차원은 기존 기본 단위의 곱·분수 표시를 유지한다. 자동 prefix 선택, 모든 복합 단위의 최적 분해, USCS/CGS 등 다른 unit system의 선택까지 구현했다고 주장하지 않는다.

단위 선택은 `result_display`의 읽기 전용 조판에만 적용한다. 물리 Quantity와 변수 binding은 바꾸지 않는다. 이미 `Scope::display`가 명시 단위 slot으로 변환한 `display_value`는 dimensionless이므로 여기서 다시 나누거나 두 번째 단위를 붙이지 않는다. 복소 결과에 단위가 붙을 때의 괄호도 유지한다. 저장된 원본 숫자를 엔진 입력에 재사용하지 않는다.

## 검증

`mathcraft-compat/tests/unit_regressions.rs`의 세 회귀가 통과했다. `d:=2 m`을 cm/mm로 표시하는 수정·Pending·새 세션 계산, m이라는 변수와 명시 UNIT m의 분리, 매개변수가 m인 중첩 함수, kHz/ns/deg/rad/s 변환, `%` 상수의 0.01 입력 배율을 검사한다. 실제 재현은 표시층에서 확인되었으므로 엔진의 단위 정책이나 contextual label을 추측으로 바꾸지 않았다.

결과 표시 테스트에는 V·Ω·W 이름 유지, 복소수+단위 괄호, L 표시 배율과 물리값 불변, 정확히 맞지 않는 기본 단위 분수 표시를 넣었고 결과 표시 16개 테스트가 통과했다. 전체 OS 검증은 [CONFORMANCE](../CONFORMANCE.md)의 통합 기록을 따른다. ignored `unit-regression-probe`는 실제 source file을 명시적으로 선택하는 headless 도구이며 원본 Mathcad나 창을 열지 않는다. `unit-regression-report.py`로 source SHA와 56개 좌표·before/after 비교를 다시 만들 수 있다.
