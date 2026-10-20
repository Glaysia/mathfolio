# 0192 — 명시적 단위 선택과 절대온도

2026-10-19. [0190 원본 계약](0190-affine-temperature-contract.md)의 유한 실수 scalar부터 연결한다. r204 고정 검증은 완료했다. Windows/peets 각각1599검사와 134MCDX·별도10XML의 XSD 814parts·64PDF 구조 검증을 통과했다. 직전 채택 기준점은 [r203](0189-radix-and-string-sharing-integration.md)이다. 고정 archive의 검증 전 문서와 이 후속 완료 기록을 구분한다.

## 바뀐 계산·원본 작성 흐름

UNIT 라벨의 °C/°F만 함수로 식별한다. source의 Scale 또는 명시 APPLY에서 입력을 Kelvin으로 바꾸고, unitOverride에서는 등록된 역변환을 사용한다. 기존 Quantity/Value wire·Δ°C/Δ°F 선형 단위·일반 산술은 유지한다. 원본 Help의 절대온도 합 설명에 없는 새 타입 거부를 추가하지 않는다. 사용자 정의·실패 binding의 우선순위도 유지한다. Celsius를1K 곱셈 단위로 등록하지 않는다.

실제 단위 메뉴는 이전에 일반 InsertSymbol을 보내고 있었다. 동명 변수 `m:=5` 뒤2와 메뉴 m을 선택하면10이 되는 [실패](../../artifacts/rea/unit-menu-command-wired-red-20261019.log)를 재현했다. 기존 검사는 실제 메뉴 대신 명시 UNIT 명령을 직접 호출해 이 경로를 놓쳤다. 지금은 메뉴와 검사에서 같은 unit_choice를 사용하며, 명시 UNIT와 원본 Scale 구조를 삽입한다. [수정 검사](../../artifacts/rea/unit-menu-command-repair-preflight-20261019.log)가 통과했다. °C/°F·온도차도 단위 메뉴에 연결한다. 문자 입력의 각도 연산자는 재해석하지 않는다.

전체 source candidate를 검증하고 실제 명령을 실행하므로 한도를 넘긴 입력은 source·캐럿·Redo를 유지한다. 단위 자리의 전체 문자 선택은 대체하고, Fraction Typeover는 기존 replacement 경로를 사용한다. 미지원 callee/partial/string/subscript 편집은 소스를 손상시키지 않고 거절한다. 활성 Area 자식·table 셀에는 기존 소유자의 명령으로 전달하고, 빈 Area에서 밖의 root 수식을 만들지 않는다. 빈 Area의 첫 단위 버튼 입력은 아직 지원 경계다.

## 검증 범위와 이력

[compat source7검사](../../artifacts/rea/affine-temperature-integration-preflight-20261019.log)·[단위 함수4검사](../../artifacts/rea/affine-temperature-unit-preflight-20261019.log)는20°C→68°F,32°F→273.15K,212°F→100°C, Δ 분리, 차원·라벨·사용자 binding·source/wire를 검사했다. 예상 수치는 원본 상수에서 도출한 수학적 기대이며 원본 엔진 실행 결과 캡처가 아니다.

앱의 실제 Text·단위 메뉴·Undo/Redo·Enter·Calculate Now·native/own/cold·벡터 PDF 흐름을 연결했다. 수동 모드에서 eval 삽입은 작성이며 Calculate Now는 보이는 범위를 계산하므로, 테스트도 새 query나 cold 문서를 frame에 표시한 뒤 명령을 보낸다. headless helper가 이 frame을 생략한 실패와 수정 기록을 보존한다. 설치 Mathcad의 온도 cached unit metadata는 UNKNOWN이다. 생성 문서는 기존 bare 표시 계수 정책과 명시 unitOverride를 사용하며 fake °C unitMonomial을 만들지 않는다. 원본 캐시 동일성으로 확대하지 않는다.

[수정된 실제 앱 흐름](../../artifacts/rea/temperature-app-flow-preflight5-20261019.log)과 [벡터 출력 probe](../../artifacts/rea/temperature-vector-smoke-preflight-20261019.log)가 통과했다. 원래 CIL 연산 순서의 f64 결과67.99999999999999/76.99999999999999는 계산 값으로 보존하고 화면 서식은68/77로 표시한다. XML 검증도 이 수치 경계를 작은 허용 오차로 비교하며 계산 값을 반올림하지 않는다. 초기 private API/SmallVec 오해와 frame이 빠진 수동 명령 fixture의 실패는 각각 preflight1–4 로그에 남아 있다.

[설치 archive 조사](../../artifacts/rea/affine-temperature-native-cache-next-20261019.md)는 원본77개와 기존 사용자3개를 확인했다. 회사 표준 정의11개 localized template에 실제 `T_std ≡ 15 scale °C`가 있고 정확한 원문 fragment·namespace·subscript·hash를 보존했다. 연결된 resultData7은 Sync지만 비어 있어 변환 결과 캐시 carrier를 증명하지 못한다. 정적 source 양성과 cached 형식 미확정을 구분한다.

함수가 직접 받는 Matrix/Range/RangeValues/complex는 첫 범위에서 unsupported이다. 기존 named-range/vectorize의 scalar lifting은 별도 상위 경로이며 Range.step에 offset을 적용하지 않는다. 일반 사용자 함수 inverse·affine XY·타 단위계·전체 입력/캐시 형식은 남아 있다. GUI·스크린샷·원본 Mathcad/CLR/COM은 사용하지 않는다.

## 최종 고정 소스 검증

[검증 JSON](../../artifacts/rea/string-values-integration-20261019-r204/validation.json)과 [manifest](../../artifacts/rea/string-values-integration-20261019-r204/source-manifest.json)의 소스524파일·제품 Rust270파일을 기준으로 채택했다. archive SHA256 `56cb9d7d23603f22bf39cf18b902fdc22042f3ca640e3db00ad2239fae188149`, manifest SHA256 `6a8f5065075e70c4e9fd9d30716d7a38ba88b1ed7d9380efddcb037ae4b85f7f`다.

Windows workspace 1597 + backend 2 = 1599, peets 1599; ignored 87는 일반 성공 수에 넣지 않았다. fmt·workspace/backend Clippy 경고0·build·reference·release와 명시 probe가 통과했다. 온도 probe는 OS별 정확히1개 검사를 실행했고, source/cache/own/PDF의 예상 경로 전체 집합을 확인했다. CPU 프로파일 두 개는 이 게이트의 실행 대상에서 제외했다.

| 산출물 | 바이트 | SHA256 |
|---|---:|---|
| [mathcraft-string-values-r204-20261019.exe](../../artifacts/rea/mathcraft-string-values-r204-20261019.exe) | 24942080 | `7314a262b8e3a0274e6c2d1e6b306aecaabed0688854842bf4621de7632f9116` |
| [string-values-r204-headless-20261019.exe](../../artifacts/rea/string-values-r204-headless-20261019.exe) | 28038144 | `2334037278af986e70a64319697e2d15cb3921bf794c6613a05cc2b3019074c0` |
| [string-values-r204-peets-evidence.tar](../../artifacts/rea/string-values-r204-peets-evidence.tar) | 5345280 | `44bfebedd0679a99160683385ed05c820886b2e912f5902e1da4839ad3b1bec1` |

계산·정적 source·생성 native/PDF 구조를 확인한 범위이며 원본 전체 cache fidelity·GPU·40% 달성을 뜻하지 않는다.
