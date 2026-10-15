# Prime 12 계산 모듈 목록과 과학 함수

2026-10-15. 사용자 지시에 따라 GUI·스크린샷·원본 프로세스 실행을 중단하고 REA 정적 분석과 Rust 계산 엔진 구현을 진행했다.

## 인터페이스를 찾은 경로

REA `inspect_managed_artifact`로 설치된 `efi*.dll`의 `Mpl.Run.ModuleInterfaceAttribute`를 확인했다. 일반 MethodDef 목록에는 직접 나타나지 않는 내장 함수가 이 속성에 이름·타입·외부 구현 경로로 선언되어 있다. 따라서 MethodDef에서 `mc_*`만 검색한 결과를 전체 함수 목록으로 쓰면 많은 기능을 놓친다.

`scripts/extract-engine-catalog.py`는 REA가 기록한 파일 SHA-256과 속성 blob SHA-256을 모두 검증한 뒤 dnfile로 해당 blob만 읽어 선언의 사실을 추출한다. 원본 어셈블리를 로드하거나 코드를 실행하지 않는다. 원본 인터페이스 텍스트와 전체 REA 출력은 ignored `artifacts/rea/engine`에만 둔다. 저장소에는 바인딩 이름, 함수/연산자/값 구분, 엔진 인자 수, optional/variadic/ORIGIN 여부, 차원 변수, 연결 대상 및 증거 ID만 남긴다. 실행 본문은 복제하지 않는다.

결과는 [계산 바인딩 목록](../spec/mathcad12-engine-catalog.json)이다. 명시적으로 읽은 20개 모듈에서 868개 바인딩, 그 중 함수형 서명 761개를 찾았다. 내부·호환용 항목도 포함되며 사용자 기능 수나 구현 완료 수가 아니다. `engine_arity`에는 숨은 ORIGIN과 묶인 optional 인자가 포함되어 사용자 호출 인자 수와 다르다. 이 목록으로 미조사 모듈이나 완전한 프로그램 기능까지 조사됐다고 주장하지 않는다.

분야는 core, Bessel, calculus, data, differential equations, finance, fitting, Fourier, graphics, image, file IO, matrix, plot, signal, Six Sigma, solver, special, statistics, symbolics, wavelets이다. 각 모듈의 SHA-256·MVID·attribute token·blob SHA-256·REA ID는 목록의 `modules`에 있다.

대표 직접 증거:

| 대상 | REA 증거 | 확인 범위 |
|---|---|---|
| `eficore.dll` interface | `ev_016bd2c29cfbda9d1596e0b3ad3b43f2bbe6bed7310aee9be644418fd2794630` | 함수 타입·단위·연결 대상 |
| `eficore.dll` members | `ev_d1a2fa460549e186f494812827de4c4bc50d2b7ca7f1858617fe9877fc866ef8` | `mc_log`·`mc_round`·`mc_signum` optional 기본값 경로 |
| `eficore_.dll`의 `Efi.Core`, `Efi.CoreExports` | `ev_9b6b49e1d2b919b7dab3cc072596d9c2593526001613ee05faacbfee9a2bfbb6` | scalar/vector/matrix CIL dispatcher 분기와 호출 경계 |
| `mplprelude.dll` members | `ev_78c59de59e5088bb057b759197b20dd92030be1470034f84a6f5c7e7d37ac0bb` | 저수준 수치 helper의 System.Math 호출; 사용자 함수 경로와 구분 |

Efi.Core artifact SHA-256 `d9a0a8e7716b71f2ccbc14af4bc48bc52bf3f53e0f1d9bbab825947c3aeffeaf`, MVID `63cb63d9-2dbe-47d5-93e9-23bde43b7a04`에 토큰을 결합한다. 선택한 타입의 managed 멤버 coverage는 complete이나, 혼합 어셈블리 전체 native 코드와 수치 알고리즘의 완전한 복원을 뜻하지 않는다.

## Rust에 반영한 계약

`scalars.rs`는 삼각·역삼각·쌍곡·역쌍곡 함수, `sinc`, `exp/ln/log`, `Re/Im/conj/arg`, `angle/atan2`, `sign/signum/csgn`, 반올림·절삭·나머지·타입 판별 함수를 독립 구현한다. 제곱근과 정수 factorial 연산도 연결한다. 복소수는 num-complex, 물리량은 기존 Quantity를 사용한다.

- `atan2(x,y)`는 `arg(x+iy)`이다. Rust의 `y.atan2(x)`에 대응하며 결과 구간은 −π 제외, +π 포함이다. `angle`은 0 이상 2π 미만이다. 두 좌표의 단위가 같아야 한다.
- `log(z)`의 기본 밑은 10이며 `log(z,b)`를 지원한다. `ln`과 구별한다.
- 소문자 `floor/ceil/trunc/round`는 dimensionless이고, 대문자 `Floor/Ceil/Trunc/Round`는 같은 차원의 quantum으로 나눈 뒤 절삭/반올림하고 단위를 복구한다. 복소수 실수부와 허수부는 각각 처리한다.
- `round(z,n)`의 optional n은 기본 0이다. 원본의 모든 이진 부동소수 tie를 재현했다고 주장하지 않는다.
- `mod` 결과 부호는 피제수와 같다. `sign(0)=0`, `signum(0)=1`, `signum(0,x)=x`이다. `csgn`은 복소 평면의 반쪽을 구분한다.
- scalar/vector dispatcher를 일반 matrix mapping과 구분한다. `Re/Im/conj`는 일반 matrix에도 적용하고 일반 과학 함수는 column vector까지만 자동 적용한다. 명시 vectorize 연산은 별도 미완료다.
- 사용자 정의와 명시 라벨이 내장 함수보다 먼저 적용된다. 새 식에서 함수 위치의 `min`을 시간 단위 min과 혼동하지 않도록 제한된 자동 라벨 선택을 수정했다. 원본의 모든 자동 라벨 문맥을 복원한 것은 아니다.

설치 도움말 `polar_angles.html`, `complex_number_functions.html`, `truncation_and_rounding_functions.html`, `sign_functions.html`, `gcd_lcm_and_mod.html`, `trigonometric_functions.html`, `inverse_trigonometric_functions.html`, `exponential_and_logarithmic_functions.html`를 CIL/인터페이스와 대조했다. 도움말은 동작 계약 자료이며 실제 신규 실행 결과로 분류하지 않는다.

## 검증과 남은 범위

`tests/scientific.rs`는 인자 순서·branch·optional 기본값·복소수·차원·배열 모양·명시 오류를 검사한다. `examples/engineering.xml`과 `tests/engineering.rs`는 행렬, 사용자 프로그램, root, 적분, 미분, 통계, 다항식 근을 한 문서에서 실행한다. 모두 자체 작성한 입력과 수학적 기대값이며 원본 실행 oracle이 아니다.

```sh
cargo run -p mathcraft-cli --locked -- features
cargo run -p mathcraft-cli --locked -- features --all
cargo run -p mathcraft-cli --locked -- mathml-run examples/engineering.xml
```

`features`는 실제 dispatcher에 연결된 바인딩만 `bounded-subset`로 표시한다. 모든 타입·정밀도·알고리즘 branch를 지원한다는 뜻이 아니다. 나머지는 명시적으로 `unimplemented`이다. 계산 목록을 런타임에서 PTC 설치 경로에 의존하지 않고 읽을 수 있다.

Bessel/특수 함수 전체, 분포·신호·FFT·영상·파일 IO·회귀·미분방정식·최적화 등 다수는 아직 구현하지 않았다. 계산 목록 발견과 구현 완료를 혼동하지 않는다. 프로그램·배열·수치 계산·통계·단위의 세부 근거와 제한은 분석 0005–0010에 기록한다.
