# Prime 12 SYSTEM 기본값과 적용 경계

2026-10-15. 원본 애플리케이션, CLR, COM, GUI 및 스크린샷을 사용하지 않았다. 기존 REA metadata, 원본 바이트를 해시 대조한 CIL decode, 설치된 `lisp/front12.lisp`와 영문 도움말을 읽었다.

## 확정한 기본값

| SYSTEM 이름 | 기본값 | 용도 |
|---|---:|---|
| `ORIGIN` | 0 | 배열 인덱스 시작값 |
| `TOL` | 0.001 | 수치 적분·미분·root 등 일부 알고리즘의 수렴 기준 |
| `CTOL` | 0.001 | solve block의 제약 허용 오차 |
| `PRNPRECISION` | 4 | WRITEPRN/APPENDPRN ASCII 출력의 유효 자릿수 |
| `PRNCOLWIDTH` | 8 | 해당 ASCII 출력의 column 폭 |

이 값들은 일반 수식 결과의 표시 자릿수를 모두 설정하는 옵션이 아니다. 특히 PRNPRECISION을 화면의 모든 숫자 표시 precision으로 사용하면 계약이 달라진다. CWD는 숫자 기본값이 아니라 현재 작업 디렉터리 문자열이다. 실제 경로를 임의의 하드코딩한 Windows 폴더로 대체하면 안 된다.

## 실제 설정 코드와 frontend 연결

`Ptc.Controls.Core.dll` SHA-256 `654a0252f699c9c9763aa4f4102488279500ed89cb8ce285a38476c2dd24a977`, MVID `b5e8e4fd-c134-4713-b1b1-eea8cddcdd9f`, 기존 REA evidence `ev_f208e991f9e1e1e33663525464eb86305e6bee7352187fed8011d74326426e64`를 사용했다. selected 타입 `Ptc.Controls.CalculationVariables`, `CalculationOptions`의 56개 file-backed 메서드를 다시 해독하고 각 raw CIL SHA-256을 metadata와 대조했다. ignored 재현 자료는 `artifacts/rea/system-deep/decode.py`, `settings-full-il.json`이다.

`CalculationVariables..cctor` 토큰 `0x060000e1`, 첫 CIL 파일 offset 5724는 WPF DependencyProperty metadata에 기본값을 실제 등록한다.

- TOL: IL_0019의 `0.001`, metadata 등록 IL_0038, `TolProperty` 저장 IL_003d.
- CTOL: IL_005b의 `0.001`, metadata 등록 IL_007a.
- ORIGIN: IL_009d의 정수 0, `IndexOriginProperty` 저장 IL_00b9.
- PRNCOLWIDTH: IL_00d7의 정수 8, metadata 등록 IL_00ee.
- PRNPRECISION: IL_0111의 정수 4, metadata 등록 IL_0128.

`HandleTolChanged`(`0x060000d9`, CIL 시작 5184), `HandleCTolChanged`(`0x060000da`, 5248), `HandleIndexOriginChanged`(`0x060000db`, 5312), PRN callback 두 개는 각각 `mc_TOL`, `mc_CTOL`, `mc_ORIGIN`, `mc_PRNCOLWIDTH`, `mc_PRNPRECISION`을 calculation-variable changed 이벤트로 보낸다. 단순히 화면 속성 이름만 비슷한 관계가 아니다.

`front12.lisp` SHA-256 `5b15dd2d9066b253200133a5b5fd79263718a9f5f5d4e520cdadda5c104e01ee`의 3402–3446행은 builtin label 집합과 `ibuiltin/set-variable`의 실제 numeric/symbolic/explicit 정의 action 갱신 경로를 보여 준다. 581–583행은 ORIGIN을 항상 0으로 정의한다. 3655–3663 및 3700–3716행은 위 다섯 SYSTEM 이름을 특수 취급하며 undefine 후 기본값으로 돌아오는 RHS를 정확히 같은 숫자로 만든다. 따라서 기본값은 단지 도움말에만 적힌 관례가 아니다.

`RandSeed` 설정 metadata의 기본값은 double 1이다(`0x060000e1` IL_017f, 첫 CIL 파일 offset 6107). 그러나 callback이 보내는 이름은 `rand_seed`이고 frontend 3415–3424행은 이를 `Seed` 함수 호출을 실행하는 부작용으로 처리한다. 이를 곧바로 worksheet의 새로운 numeric SYSTEM 상수 `SEED=1`로 구현하면 안 된다.

`CalculationVariables..ctor` (`0x060000c8`, 첫 CIL offset 4777)는 `Environment.CurrentDirectory`에 경로 구분자를 붙여 CWD 초기값을 만든다. callback `HandleCurrentWorkingDirectoryChanged`(`0x060000de`, 5504)는 경로 escaping 후 `mc_CWD`를 보낸다. Rust 구현은 플랫폼의 경로 표현을 고려해야 한다.

## 검증 제약의 확인 범위

설치 도움말 `to_set_the_array_origin.html`은 worksheet 내 ORIGIN을 양수/음수 정수로 재정의할 수 있다고 명시한다. 설정 메뉴에서 0 또는 1만 선택하는 UI와 worksheet의 허용 정수 범위를 구별해야 한다. 기존 Rust의 실수·허수·단위·fraction 검사와 별도로 **음수 ORIGIN을 금지할 근거는 없다**.

위 DependencyProperty 등록은 `PropertyMetadata`와 changed callback을 사용하며, 이 경로에는 별도의 ValidateValue/CoerceValue callback이나 수치 상·하한 검사가 없다. Tol/CTol/RandSeed의 설정 타입은 double이고 ORIGIN/PRN 두 항목의 설정 타입은 integer이다. 하지만 이것이 worksheet 내 임의의 재정의가 모든 함수에서 허용된다는 뜻은 아니다. root/적분은 자체 소비 지점에서 scalar·차원·부호 등을 검사하며, PRN 출력은 파일 함수에서 precision/width를 해석한다.

원본의 TOL/CTOL 허용 최솟값·최댓값, PRNPRECISION/PRNCOLWIDTH의 정확한 허용 범위 및 오류 family는 이번 조사에서 확정하지 못했다. 도움말의 적분 권장 TOL `1e−4`~`1e−6`은 알고리즘 사용 권고이며 validation 상·하한으로 사용하면 안 된다. 코드에서 TOL을 임의로 잘라내거나 잘못된 binding을 기본값으로 조용히 대체해서는 안 된다.

`built-in_constants_and_variables.html`의 System Variables 표는 위 다섯 기본값과 CWD의 string 성격을 명시한다. `numerical_integration_methods.html`은 TOL이 실제 알고리즘 종료 기준에 관여함을 설명한다. 문자열과 PRN 출력, random seed의 부작용은 현재 numeric Quantity 타입으로 대체할 수 없는 별도 기능이다.

## 현재 Scope에 적용할 최소 계약

기존 explicit binding 조회를 먼저 수행한 후, SYSTEM 라벨의 위 다섯 이름에 한해 scalar 기본값을 제공하면 `TOL=` 자체가 undefined인 문제를 해결할 수 있다. failed/redefined binding은 fallback으로 덮지 않는다. typed identifier 자동 분류도 이 정확한 SYSTEM 이름을 인지해야 한다. source의 VARIABLE 이름과 SYSTEM binding을 단순 문자열로 합치지 않는다. 기존 ORIGIN runtime reference에서 VARIABLE 정의와 인덱스 시스템 binding은 구별됨을 확인했으며, 다른 이름까지 자동 병합하는 규칙은 추가 증거 없이 확대하지 않는다.

문서의 calculation settings가 기본값을 바꿀 수 있으므로 이 fallback은 설정값을 전달하기 전의 기본 상태이다. helper 내부의 TOL 기본값과 Scope 조회값이 서로 다른 두 저장소가 되어서는 안 된다. 함수의 검증 오류나 failed TOL 정의를 numeric helper가 `0.001`로 바꾸는 것도 잘못이다.

재부팅 후 구현에서 Scope 조회에 위 다섯 기본값을 연결하고 numeric/solve helper의 별도 기본값 경로를 제거했다. 명시적 정의와 실패한 정의를 먼저 조회하므로 fallback이 오류를 덮지 않는다. `tests/system_defaults.rs`에서 기본값의 직접 평가, VARIABLE 라벨과의 분리, 정상·실패한 재정의의 우선순위, Seed/CWD를 숫자로 발명하지 않는 동작을 확인한다. 문서별 calculation settings 전달과 전체 validation/error 호환은 미완료다.
