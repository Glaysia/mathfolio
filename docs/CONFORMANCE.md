# Mathcad 호환성 구현과 측정

대상은 설치된 PTC Mathcad Express Prime 12.0.0.1이다. 오픈소스 Mathcad의 작성 경험을 목표로 한다. [작성 경험 기준](DESIGN.md)에 시나리오·원본 근거·독립 구현을 연결한다. 원본 DLL·Lisp·폰트·리소스를 배포물에 넣지 않는다. REA와 설치 파일은 개발 분석에만 쓰고 Rust 실행에 필요하지 않다.

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

`native-copy`는 새 파일만 만들고 재오픈해 각 항목 이름·내용·압축 방식을 비교한다. 압축 컨테이너 전체 바이트 및 모든 ZIP extra field의 동일성은 보장하지 않는다. ZIP64·암호화·다중 볼륨·일부 압축 방식은 명시적으로 거부한다. 실제 detune의 33개 항목 보존을 확인했다. 편집 export는 아래 범위로 구현했으며 원본 앱 재열기는 검증하지 않았다. 원본 문서는 변경하지 않았다.

`native-run`은 캐시를 읽어 답으로 사용하지 않는 새 Rust 계산이다. 기본 수식·정의에 더해 행렬 연산, ranges, 프로그램, 수치 미분·적분·root, 일부 통계, scalar equality 풀이블록, 제한된 유리수·다항식 상징 계산을 연결했다. SI 단위 212개·물리 상수 15개와 SYSTEM 5개 기본값을 반영했다. 정확한 타입·알고리즘 범위는 [ENGINE.md](ENGINE.md)와 분석 0005–0013에 기록한다. 영역 계층 전체 scope, 문자열, 전체 built-in/unit catalog, affine 단위, 전체 풀이·CAS는 미완료다. 미지원 구조를 0이나 이전 값으로 대체하지 않는다.

원본의 `label-is-contextual=true`는 명시 라벨과 다르다. 저장 정보는 보존하고 `worksheet_xml`이 만드는 계산 사본에서 제한된 구조·이름 규칙으로 다시 분류한다. 직접 MathML 실행에서 해결되지 않은 문맥 라벨은 cached label을 강제하지 않고 미지원으로 보고한다. 전체 자유 식별자 해석 우선순위는 미확정이다. [0016](analysis/0016-contextual-labels.md) 참고.

`mathcraft-editor`는 안정 ID의 AST arena, 캐럿·부분식 선택, 분수·지수·괄호·이항 연산 삽입, scope 방향·확장 이력·진입 커서 복원, 괄호 나가기, undo/redo와 명령 단위 편집 그룹을 구현한다. 선택한 부분식의 좌우 방향에 따른 피연산자 위치를 반영했다. 근거는 `docs/spec/mathcad12-editor-contract.json`, 분석 0002·0014다. 부분 피연산자 범위와 전체 우선순위·키 전이는 미완료다.

앱 본체는 빈 종이 위에 수식·텍스트를 배치하는 워크시트다. 수식 입력→구조 편집→정의/평가→동일 영역의 결과 표시→위치 이동에 따른 재계산→저장/재열기가 연결된다. 정의 RHS의 `=`와 반복 평가 요청을 분리했고, 선택 치환·붙여넣기·텍스트 이벤트 Undo를 검사한다. `:` shortcut과 숫자 뒤 단위 입력 등은 제한된 정책이며 원본 전체 키 문맥을 재현하지 않는다. 실제 창을 띄우지 않는 egui context와 순수 상태 전이 테스트를 사용한다. 실행 파일은 `target/debug/mathcraft.exe`다.

`mathcraft-document::canvas` v2는 수식의 PortableTree와 좌표·텍스트·opaque 영역을 저장한다. 내부 arena/caret/undo는 저장하지 않는다. MCDX를 가져오면 원본 ZIP 바이트도 보존한다. 편집 불가/계산 미지원 영역은 raw로 남기고, 계산 의미가 있는 미지원 math wrapper 속성을 제거하여 실행하지 않는다. 자체 `.mathcraft` v2 저장과 새 파일로 MCDX 편집 export를 지원한다. export는 원본 parts·관계를 보존하며 계산 캐시를 Pending으로 무효화한다. 원본 XSD에 대한 생성 구조 검사와 Rust 재열기를 검사했고, 원본 앱 재열기는 검증하지 않았다.

검증 결과는 아래 최근 검증 기록에 별도로 남긴다. 원본 GUI 재검증과 한글 IME 검증은 headless 검사에 포함되지 않는다. 새 수학 기능의 정적 계약 테스트와 기존 원본 실행 38개 표본을 구분한다.

## 다음 완료 조건

구조 편집의 부분 피연산자 범위·전체 원본 키 전이, IME·OS 클립보드, 선택·이동·계산 오류의 AST 위치 표시를 확장해야 한다. MCDX의 편집·캐시 무효화·관계 보존을 더 넓은 문서에서 검증하고 원본에서 다시 열어야 한다. 고급 계산과 전체 풀이/CAS·affine 단위도 미완료다. 제한된 기준 사례나 단위 테스트를 이유로 제품이 완성됐다고 판단하지 않는다.

## 최근 검증: 2026-10-15 재부팅 후

- Windows: workspace fmt·Clippy(`-D warnings`)·테스트·빌드 통과. 전체 190개 통과 후 직접 MathML 문맥 라벨 거부 회귀 테스트 1개를 추가하여 해당 8개 테스트도 통과했다. 이 시점의 테스트 합계는 191개다. 이전 문서의 합산 오류를 정정했다.
- peets(Kubuntu 26.04): 같은 소스 사본 `/home/peets/Projects/mathcraft-ux-20261015`에서 `scripts/check.sh` 전체 통과. 기존 소스 폴더를 덮어쓰지 않았으며 빌드 산출물 디렉터리만 재사용했다.
- 양쪽에서 기존 원본 38개 참조가 저장 좌표·입력 격자 좌표 두 경로 모두 일치했다. peets의 `engineering.xml` 20개 영역은 오류 0개이며, 값은 별도 engineering 통합 테스트로 검사한다.
- 설치 원본에 대한 unit facts 추출 `--check`는 212개 단위·15개 상수 사실과 일치했다. 피드백 분석은 설치 DLL 5개 및 CIL 메서드 123개의 원본 바이트 해시를 재검증했다.
- 원본 앱, GUI, 스크린샷, COM을 새로 실행하지 않았다. 이 검증은 전체 Mathcad 호환성이나 실제 IME 사용감을 입증하지 않는다.

## 작성 UI 개선 검증: 2026-10-15

Windows와 peets의 `/home/peets/Projects/mathcraft-ui-20261015` 소스 사본에서 fmt, workspace Clippy(`-D warnings`), 전체 테스트 **207개**, 빌드가 통과했다. 앱 테스트는 45개, editor 테스트는 19개이며 나머지는 기존 계산·문서 회귀 검사다. Windows 실행 파일 `target/debug/mathcraft.exe`도 갱신했다.

새 검사는 종이 전용 배율과 50%·100%·200% 클릭 좌표, 문자 드래그·UTF-16 선택·치환·삭제, 기하 기반 분수·행렬 수직 이동, 수식·텍스트 Undo 후 커서 위치에서 연속 입력, Esc→영역 이동→Enter, 도구막대가 있는 상태의 Tab 이동, 메뉴 열기→Esc→입력 복귀, IME 임시·확정·취소 이벤트를 포함한다. 분수 도구가 기존 선택 피연산자를 버리지 않는 동작도 검사했다.

UI 구조·타이포그래피·기하 이동은 독립적인 제품 정책이며 원본 전체 외형/키 전이와 같다고 주장하지 않는다. 실제 창·화면 캡처·원본 실행은 사용하지 않았다. 실제 Windows/Linux IME 후보창, OS 클립보드, 고해상도 화면에서의 최종 사용감은 실기 검증이 남아 있다. 구현 흐름은 [작성 경험 기준](DESIGN.md)과 [사용법](PROTOTYPE.md)에 기록했다.

## 기본 공학 계산서 단계 검증: 2026-10-15

Windows와 peets의 /home/peets/Projects/mathcraft-m20-20261015에서 workspace fmt, Clippy(경고 금지), 전체 244개 테스트, 빌드를 통과했다. 앱 71개, editor 25개, document 28개 테스트를 포함하며 기존 원본 38개 참조의 두 좌표 경로도 유지된다. 실행 파일은 Windows target/debug/mathcraft.exe, Linux /home/peets/Projects/mathcraft/target/debug/mathcraft다.

새 검증은 함수·범위·도표 작성과 정의 수정/Undo에 따른 재계산, 축 편집 후 포커스, 행렬 행·열 삽입/삭제와 Shift 단축키, 서식의 저장·복사·Undo, 표시 반올림과 원본 계산값 분리, source hit ID, 확대 글꼴의 IME 좌표, 긴 행렬의 스크롤 범위, 숫자 일반 형식·0 표시 경계를 다룬다. 예제의 F=19.6133 N, det(A)=5, f(2)=8도 고정 검사한다.

수식 서식의 MCDX 출력은 설치본 worksheet50.xsd를 사용해 구조 검사를 통과했다. 누락된 XML namespace schemaLocation에만 오프라인 표준 XML 속성 선언을 연결했고 제품 스키마 선언은 유지했다. 원본 앱으로 재열거나 화면을 대조한 결과가 아니다. 도표의 MCDX 내보내기는 명시적으로 거부하여 저장 누락을 막는다.

이 단계에서 GUI·스크린샷·원본 실행은 사용하지 않았다. 전체 제품의 20%라는 수치는 계측한 호환율이 아니라 기본 계산서 작성 흐름을 기준으로 한 개발 규모의 추정이다.
