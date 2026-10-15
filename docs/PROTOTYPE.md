# Mathcraft 프로토타입

2026-10-15 기준. Rust 독립 앱이며 Mathcad 설치나 라이브러리를 실행 의존성으로 사용하지 않는다.

이 문서는 초기 행 기반 계산기 실험의 사용법이다. 이 UI는 목표 제품으로 인정하지 않으며, 현재 개발 기준은 [Mathcad 호환성](CONFORMANCE.md), 자유 배치 워크시트와 구조 수식 편집이다. 새로운 native/compat/editor 계층은 기존 문자열 importer와 분리되어 있다.

## 실행

Windows에서 `target/debug/mathcraft-app.exe`를 실행하거나 아래 명령을 사용한다.

```powershell
./scripts/run.ps1
./scripts/run.ps1 -Open 'C:\Users\harry\Downloads\detune.mcdx'
```

Linux에서는 `cargo run -p mathcraft-app --locked`를 KDE 터미널에서 실행한다. Windows는 WGPU, Linux는 Glow 렌더러를 사용한다. 이 Windows 환경의 OpenGL 컨텍스트 생성 실패 때문에 WGPU로 전환했고 앱 자체 PNG 캡처로 실제 표시를 확인했다.

## 기능과 입력

- 수식·메모 행 편집, 위에서 아래로 재계산, 변수 재정의, 행 추가·삭제·이동.
- 실수/복소수 연산, SI 차원 검사·접두어·변환, 사용자 함수.
- sqrt, abs, re/im, conj, 삼각함수·역삼각함수, exp, ln, log/log10.
- 변수 목록, 실수·무차원 함수 그래프와 확대·이동.
- 자체 `.mathcraft` 저장·열기, 실행 취소·다시 실행(최대 50개 변경).
- 관찰한 Prime 12 worksheet50 MCDX의 읽기 전용 부분 가져오기: 수식·단위·아래첨자·XAML 한글 메모·영역 좌표·저장 캐시.

```text
f := 360 kHz -> kHz
omega := 2*pi*f -> rad/s
L := 15 uH -> uH
C := 1/(omega^2*L) -> nF
Z := 50 ohm + i*omega*L -> ohm
abs(Z) -> ohm
distance := 3 m + 20 cm -> cm
response(x) := sin(x)*exp(-0.15*x)
response(pi/2)
```

`->`가 없으면 SI 기본 단위 조합으로 표시한다. `i`/`j`는 이 문자열 문법의 허수 단위이며 원본 MathML의 CONSTANT `i`와 같지 않다. `pi`/`π`는 원주율이다. 아래첨자는 `f_0`, 주석은 `#`로 입력한다. 함수의 외부 변수는 정의 시점의 환경에 묶도록 원본 결과에 맞춰 수정했다. 이 문법의 자동 이름 해석·함수 오류 처리 전체가 Mathcad와 같다는 뜻은 아니다.

`Open / Import`로 `.mathcraft` 또는 `.mcdx`를 연다. 가져온 문서는 `.mathcraft`로 별도 저장하며 원본 `.mcdx`를 덮어쓰지 않는다. Ctrl+S 저장, Ctrl+Z 실행 취소, Ctrl+Y 다시 실행. 수정 후 새 문서·열기·종료를 요청하면 저장 여부를 묻는다. 저장/열기 대화상자 전체 시나리오와 IME 조합 입력은 아직 검증 전이다.

## CLI

```powershell
cargo run -p mathcraft-cli --locked -- eval 'x := 3 m; x -> cm'
cargo run -p mathcraft-cli --locked -- run examples/resonant-circuit.mathcraft
cargo run -p mathcraft-cli --locked -- run 'C:\Users\harry\Downloads\detune.mcdx' --json
cargo run -p mathcraft-cli --locked -- import INPUT.mcdx OUTPUT.mathcraft
```

`run --json`은 행별 오류와 저장 캐시 비교를 반환한다. 명령 종료 코드만으로 계산 성공 여부를 판단하지 말고 `errors` 필드를 확인한다.

## 검증

- Windows/peets에서 fmt, Clippy(-D warnings), workspace 빌드, 테스트 13개 통과.
- Windows GUI 프레임 렌더링·앱 자체 캡처·한글 메모 표시 확인. Windows 창 캡처 플러그인은 시간 초과로 실패했다.
- peets는 빌드·테스트·데모 CLI 확인. 실제 KDE GUI/IME/GPU 검증은 미완료다.
- 비공개 표본 detune: 87개 영역(수식 66·메모 21), 계산 오류 0. 수치 캐시 51개 중 50개가 `max(1e-12, |expected|*1e-8)` 오차 안에서 일치했다. 새로운 Mathcad 실행과 비교한 결과가 아니다.
- 불일치 1개: region 20/result 14의 `M := 17.704 μH` 정의와 캐시 `0.000171445 henry`. 캐시는 Synchronized로 표시되지만 현재 XML과 같은 값인지 확인되지 않았고 원인은 미확정이다.

자유 배치/2D 수식 커서, 행렬, 범위 변수, 상징 연산, 미적분/풀이 블록, 섭씨·화씨 같은 affine 단위, 원본 MCDX 왕복 저장은 미지원이다. 수식 4,096 bytes/256 tokens, 중첩 64단계, 문서 1,000행 등의 한도가 있다. 미지원 XML은 오류 행으로 표시한다. 일반적인 MCDX 파일 전체의 호환성이나 원본과 동일한 평가 순서·정밀도는 아직 입증하지 않았다.
