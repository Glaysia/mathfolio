# Mathcad 호환성 구현과 측정

대상은 설치된 PTC Mathcad Express Prime 12.0.0.1이다. 오픈소스 Mathcad를 목표로 하며 기존 행 기반 GUI는 초기 실험이다. 원본 DLL·Lisp·폰트·리소스를 배포물에 넣지 않는다. REA와 설치 파일은 개발 분석에만 쓰고 Rust 실행에 필요하지 않다.

## 재현 가능한 기준

`tests/reference/mathcad12.json`은 우리가 생성한 최소 워크시트 38개를 원본에서 새로 열고 계산 완료를 확인한 결과다. 입력/출력 MCDX SHA-256, 원본 버전, 생성한 MathML, 저장된 좌표와 resultRef, 표시 수치·오류 태그·경고를 포함한다. ZIP 원본과 원본 제품 리소스는 Git에 넣지 않는다.

```sh
cargo run -p mathcraft-cli --locked -- conformance tests/reference/mathcad12.json
```

`mathcraft-compat`는 문자열로 평탄화하지 않고 MathML 구조와 라벨을 직접 평가한다. 비교 항목은 표시 실수/허수/열벡터, 오류 family, 이전 오류 참조의 존재 여부, 재정의 경고다. 수치 허용 오차는 `max(1e-12, abs(expected)*1e-8)`이다. 원본이 저장한 좌표와 원본에 입력한 좌표+문서 격자 설정을 각각 실행한다. Pending·실패·비어 있는 결과를 정답으로 세지 않고, 차이가 있으면 CLI는 비정상 종료한다.

현재 38개 표본이 두 경로에서 일치한다. 이 수치는 전체 Mathcad 호환율이 아니다. 정확한 오류 인자/노드 handle/ID, 출력 차원 자체, 모든 숫자 branch와 정밀도, 캐시·취소·수동 계산 수명 주기는 이 수치에 포함되지 않는다. 동일 위치 두 영역의 일반적인 tie 순서도 아직 확정하지 않았다.

확인한 차이에서 구현을 고쳤다.

- `a:=2; f(x):=a*x; a:=3; f(4)`는 **8**이다. 함수가 정의 당시 환경을 가진다.
- `0^0`은 이 원본에서 **1**이다.
- 계산 순서는 XML 저장 순서와 분리되어 위치에 연결된다. 저장 시 region/result ID도 바뀔 수 있으므로 출력 worksheet의 resultRef로 결과를 연결한다.
- 문서를 열 때 저장된 종이/격자의 교차점에 배치한다. 20과20+1e-8이 같은 격자로 이동하는 사례를 통해 단순 Y 정렬의 오답을 발견했다. REA로 로딩→AddItemAtLocation→AlignmentGrid 경로를 확인해 수정했다. 정확히 절반이면 아래쪽 교차점으로 맞춘다.
- VARIABLE ORIGIN 재정의와 SYSTEM ORIGIN은 같지 않다. CONSTANT i는 실제 허수 literal이 아니며 원본에서 undefined다. 표본 이름이 imaginary-square/complex-abs여도 그 입력의 실제 결과는 undefined로 남긴다.
- 실패한 재정의는 이전 정상값을 조용히 되살리지 않는다. 미지원 RHS와 중복 global도 종속 계산을 오류로 막는다.

## 새 기반 계층

`mathcraft-document::native`는 압축 항목의 원본 바이트를 유지하며 영역 계층, namespace·라벨·아래첨자, 좌표·크기·align 속성, 캐시 상태와 오류를 조회한다. 알 수 없는 XML/binary/relationship도 보존한다.

```sh
cargo run -p mathcraft-cli --locked -- native-inspect INPUT.mcdx
cargo run -p mathcraft-cli --locked -- native-copy INPUT.mcdx NEW.mcdx
cargo run -p mathcraft-cli --locked -- native-run INPUT.mcdx
```

`native-copy`는 새 파일만 만들고 재오픈해 각 항목 이름·내용·압축 방식을 비교한다. 압축 컨테이너 전체 바이트 및 모든 ZIP extra field의 동일성은 보장하지 않는다. ZIP64·암호화·다중 볼륨·일부 압축 방식은 명시적으로 거부한다. 실제 detune의 33개 항목 보존을 확인했다. **편집 저장은 아직 없고 원본 앱에서 새 복사본 재열기도 아직 검증하지 않았다.** 원본 문서는 변경하지 않았다.

`native-run`은 캐시를 읽어 답으로 사용하지 않는 새 Rust 계산이다. 현재 top-level 수식, 명시적 라벨, 기본 산술·함수, 지역/전역 정의 일부, 열벡터와 인덱스, SI 단위 일부만 지원한다. 자동 라벨, 일반 행렬 연산, 영역 계층 scope, 전체 built-in/unit catalog, 문자열, ranges, 프로그래밍/풀이 블록, CAS, affine 단위는 미완료다. 미지원 구조를 0이나 이전 값으로 대체하지 않는다.

`mathcraft-editor`는 안정 ID의 AST arena, 캐럿·부분식 선택, 분수·지수·괄호·이항 연산 삽입, scope 확장, 괄호 나가기, undo/redo와 분수 배치식을 구현한다. 근거는 `docs/spec/mathcad12-editor-contract.json`과 분석 0002다. 원본에서 아직 대조하지 않은 키 전이·선택 wrapping·지수 배치는 독립 구현 정책이며 호환성 검증이 남아 있다.

앱 본체는 빈 종이 위에 수식·텍스트를 배치하는 워크시트로 바뀌었다. 수식 입력→구조 편집→정의/평가→동일 영역의 결과 표시→위치 이동에 따른 재계산→저장/재열기가 연결된다. `:`·`=`와 숫자 뒤 단위 입력은 현재 제한된 독립 정책이며 원본 전체 키 문맥을 재현한 것은 아니다. 실제 창을 띄우지 않고 egui context의 레이아웃·tessellation·포커스·연속 문자 입력 및 문서 수명 주기를 검증했다. 새 실행 파일은 `target/debug/mathcraft.exe`다.

`mathcraft-document::canvas` v2는 수식의 PortableTree와 좌표·텍스트·opaque 영역을 저장한다. 내부 arena/caret/undo는 저장하지 않는다. MCDX를 가져오면 원본 ZIP 바이트도 보존한다. 편집 불가/계산 미지원 영역은 raw로 남기고, 계산 의미가 있는 미지원 math wrapper 속성을 제거하여 실행하지 않는다. 현재 편집 저장은 자체 `.mathcraft` v2이며 MCDX로 편집 내용을 내보내기는 미완료다.

2026-10-15 최종 검증: Windows 및 peets(Kubuntu 26.04)에서 fmt, 전체 workspace Clippy(`-D warnings`), 테스트 67개, 전체 빌드와 원본 38개 참조 비교가 통과했다. 원본 GUI 재검증과 한글 IME 검증은 이 검사에 포함되지 않는다.

## 다음 완료 조건

구조 편집의 전체 원본 키 전이, IME·클립보드, 선택·이동·계산 오류의 AST 위치 표시를 확장해야 한다. MCDX의 편집·캐시 무효화·관계 보존을 검증하고 원본에서 다시 열어야 한다. ranges·일반 행렬·프로그래밍·풀이/CAS·유리수/affine 단위를 확장해야 한다. 현재 제한된 기준 사례나 단위 테스트를 이유로 제품이 완성됐다고 판단하지 않는다.
