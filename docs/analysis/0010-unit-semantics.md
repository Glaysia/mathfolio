# Prime 12 SI 단위 수치·차원의 독립 정적 추출

2026-10-15. 원본 프로그램, CLR, COM, GUI 및 스크린샷을 사용하지 않았다. `scripts/extract-unit-facts.py`는 REA가 관찰한 file-backed CIL과 원본 파일의 바이트 해시를 대조하고, 제한된 순수 상수 표현만 Python으로 계산한다. 원본 메서드 호출이나 어셈블리 로드는 하지 않는다.

## 확인한 원본

| 대상 | SHA-256 | MVID / REA evidence |
|---|---|---|
| `mcdstaticunitsystem.dll` | `5ba19b272cbbeb27c3e171a4bb83d1d14ae8c518cc9271810276d5eee699cb9b` | `5c2138ec-45cc-477d-a2ba-64a604e33781`; `ev_2ef822c8cdf3873a1c829ef26a26aa9c2e13f4c5c6aae03c13c90bbfd0e7f7dc` |
| `mcdstaticunitsystemsi.dll` | `8a3665f49f25c8ca83c614ec5c934d194fab2685fe396b86517d56791a0d2a01` | `8ea0c61d-7c6f-4cfa-8c55-7ed25f3c6b51`; `ev_516f379510019ca27af078efe7f8a95c4f78cd4a34dfebaa31d3080a28ab592c` |

base `values.Update` (`0x06000001`)는 CIL 4,411 bytes, raw CIL SHA-256 `9c2af56d48235acaa7635cc82e60d6d33f5a862f262ed4ea7d96afd9d480b1b6`이다. SI `values.Update` (`0x06000001`)는 5,396 bytes, raw CIL SHA-256 `ef8954ea5b78a57ce90e2dcb5f0a067af9278398e8b274d3cbfe523b7a50bd32`이다. base helper까지 실제 추출에 필요한 16개 base 메서드와 1개 SI 메서드의 CIL 해시를 모두 검증한다. 아직 읽지 않은 다른 코드의 동작까지 완료했다고 주장하지 않는다.

dimension은 임의의 unit 이름 추정으로 붙이지 않는다. SI 어셈블리의 `Mpl.Run.ModuleInterfaceAttribute`, token `0x0c000003`, attribute row file offset 8636의 Blob을 원본 PE에서 읽었다. REA attribute evidence `ev_3f61c2629ad3dc261f954f42b9a14424ed8e77f0d69a96d284bff5f15d71dd88`의 전체 Blob SHA-256 `aed766451ba625196abb9c89f0446cdc01f8e62e13258c069768a7c0a38f4467`과 동일함을 확인했다. 인증한 UTF-8 interface text SHA-256은 `98a07db124839a9664034d6975be51b82b1ac7ffceb7a78dde9f5c5e5659671e`이다. 선택한 interface 수치 선언의 차원 exponent를 **60,000으로 나누어** Quantity 차원 벡터로 변환한다. 원본은 Money까지 8개 차원을 가지고 Rust의 기존 Quantity는 7개만 가진다.

## 추출기가 허용하는 동작

실제 CIL 명령은 처음부터 끝까지 읽고 raw hash를 확인한다. 허용 목록은 literal load, argument/static field load, field store, boxing, `tail.`, return, 고정 길이 객체 배열과 constructor 기반 메타데이터 생성이다. 숫자 연산은 확인한 `MplPrelude` multiply/divide/sqrt와 base의 square/cubic/prefix helper를 제한된 stack interpreter로 계산한다. 알 수 없는 opcode, call, field owner, stack shape, 비유한 숫자는 즉시 실패한다. 숫자 fact에 영향을 줄 수 있는 모르는 코드를 건너뛰어 성공시키지 않는다.

SI의 `srv`는 원본의 real numeric payload 경계로 모델링한다. `unit`, `unit_system`, `unit_system_group`, list constructor는 표시/시스템 메타데이터로만 추적하며 숫자로 바꾸지 않는다. 이를 `1`이나 `0`으로 대체하지 않는다. base static field 152개와 SI static field 229개를 처리한 뒤 인증된 interface의 numeric `mc_*` 선언만 fact로 뽑았다. 결과는 227개 이름으로, 단위 212개와 물리 상수 15개이다. Money `¤`는 별도로 제외 기록을 남긴다. JSON의 각 fact에는 SI field token, Update의 IL offset 및 raw CIL hash가 있다.

이름의 `_2E_`, `_5F_` 같은 두 자리 hex escape를 각각 `.` 및 `_`로 해독한다. 예를 들어 `mc_e_2E_c`는 `e.c`, `mc_fl_5F_oz`는 `fl_oz`이다. 이 파일명의 dot을 editor의 literal subscript identity와 동일시할 근거는 이번 경로에서 확정하지 못했다. 따라서 원형 dotted name을 fact로 보존하고 임의의 plain underscore 또는 rich subscript 별칭은 추가하지 않는다.

## 구현과 중요한 충돌

`docs/spec/mathcad12-si-units.json`은 수치·차원·증거 좌표만 담은 독립 fact 파일이다. 원본 CIL, MPL source 및 proprietary 원문은 저장소에 복제하지 않았다. `crates/mathcraft-core/src/units.rs`는 이 fact에서 생성한 Rust 조회 표이다. `lookup`/`evaluate`/`names`는 UNIT 이름만, `constant_lookup`/`constant_names`는 물리 상수만 제공한다. generic prefix와 기존 영어 별칭은 호출하는 상위 계층이 담당한다.

원본 SI와 기존 작은 prototype 표가 실제로 달랐던 사례:

| 이름 | 원본 수치·의미 |
|---|---|
| `g` | `9.80665 m/s²`, 표준 중력. gram이 아님. SI field `0x04000090`, IL offset 3850 |
| `gm` | `0.001 kg`, gram. SI field `0x04000089`, IL offset 3705 |
| `mg` | `1e−6 kg`, 명시된 milligram 이름. `m` prefix를 잘못된 `g` 의미에 적용하면 안 됨 |
| `G` | `1e−4 tesla`, gauss. 중력 상수가 아님 |
| `cal` / `cal15` | 각각 `4.1868 J` / `4.1858 J`; calorie 정의를 합치지 않음 |
| `BTU` / `Btu` | `1055.05585262 J` |
| `psi` | `6894.757293168361 Pa`; field `0x0400003a`, IL offset 2180 |
| `Δ°C` / `Δ°F` | 온도 **차이**의 비율 1 / 5⁄9. 절대 온도 affine 변환으로 사용하지 않음 |
| `h` | `6.62607015e−34 J·s`, Planck 상수. hour 단위는 `hr`. 물리 상수와 UNIT namespace를 분리 |
| `c` | `299792458 m/s`, 광속. centi prefix와 같은 이름 binding으로 합치지 않음 |

정확한 원본 이름을 기존 fallback보다 먼저 조회해야 `g` 등 충돌을 해결할 수 있다. 기존 generic prefix fallback 정책 자체가 Prime의 모든 허용/금지 조합과 동일하다고 주장하지 않는다. 자동 라벨 추론에서 h/c/k 등 물리 상수를 예약하는 정책도 원본 contextual label inference의 완전한 구현은 아니다. 명시된 원본 라벨은 계속 보존해야 한다.

## 검증 및 남은 범위

`cargo test -p mathcraft-core --locked --test si_units`의 5개 테스트는 중력/gram/gauss 구분, 실제 unit() 통합 우선순위, 1 ft=12 in과 mile/psi/knot의 물리 관계, calorie 정의 차이, delta temperature, 물리 상수 namespace, 유일한 212개 unit 및 15개 constant, 양수·유한 차원과 Money 제외를 검증하며 통과했다. Planck/광속/전자 전하의 값과 차원도 별도로 확인한다. 이 검증은 신규 원본 실행 비교가 아니다.

재생성은 설치된 원본 파일과 ignored REA evidence가 있을 때 `python scripts/extract-unit-facts.py`로 수행한다. `--evidence-root`, `--output`, `--rust`로 새 출력 위치를 지정할 수 있다. `--check`는 파일을 바꾸지 않고 원본부터 다시 fold한 JSON과 Rust 표가 기존 결과와 같은지 검사한다(Rust의 formatting whitespace만 무시). `--check` 및 core 전체 13개 테스트가 통과했다. 원본 바이트나 evidence hash가 달라지면 기존 사실을 덮어쓰는 추측 추출을 하지 않고 실패한다.

Money, 절대 온도 offset, nonlinear unit functions, 다른 unit system, 전체 prefix catalog 및 physical constant의 모든 표시 별칭은 미지원이다. SI의 저장된 수치가 외부의 더 최신 표준과 다르더라도 여기서는 **설치된 Prime 12 빌드의 수치 계약**을 보존한다.
