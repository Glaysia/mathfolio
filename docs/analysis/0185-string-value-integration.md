# 문자열 수식·설정·출력 통합 검증

2026-10-19. [0184 원본 계약](0184-string-value-contract.md)과 [0183 입력 비용](0183-typed-xy-validation-receipt-cost.md)을 후속 통합했다. r200의 양 OS 일반 검사는 각각1529개(workspace+backend), XML634개 parts/별도 문서와60PDF의 구조 검증을 통과했지만 후속 결과 캐시·도표 String 검토 때문에 완료 checkpoint로 채택하지 않았다. [0186](0186-string-cache-and-plot-source.md)의 보완 검증을 완료했으며, 현재 완료 checkpoint는 r202다. GUI·스크린샷·원본 runtime 실행은 없다.

[formula 회귀](../../crates/mathfolio-app/src/formula/string_literal_tests.rs)는 실제 Text/Key, literal 내부 수학 gesture 차단·문자 `=`의 비계산·선택·삭제·Undo, 내부/외곽 화살표와 따옴표/body 캐럿 기하, raw whitespace/supplementary XML 왕복을 검사한다. CmdShiftZ가 내부에서 무한대 구조를 넣던 오류는 plain CmdZ/CmdY와 구분해 차단했다. source 내부에서 따옴표는 닫기 명령이 아니다.

[문자열 컨트롤러 흐름](../../crates/mathfolio-app/src/worksheet_ui/string_value_integration.rs)은 실제 `s:=`와 quote/body 입력, Enter 확정, concat·빈 String 계산, own/native 저장, cold 표시의 입력 실행0, 본문 수정 중 이전 결과 유지와 명시 확정/계산, native 결과 literal 및 PDF까지 연결한다. 일반 worksheet 수식은 Text 명령을 즉시 Undo 이력에 보관하면서 계산 입력의 확정은 따로 관리한다. plot의 owner-level draft journal과 같은 이력 정책이라고 가정하지 않는다.

[문서 설정 회귀](../../crates/mathfolio-app/src/worksheet_ui/string_settings_tests.rs)는 실제 package flag와 ORIGIN1을 direct·own reopen·background 계산에 전달한다. 잘못된 flag에서는 숫자/strlen을 유지하고 substr/search를 false 인덱스로 계산하지 않는다. 새 source/meta를 저장했다는 사실과 native 전체 의미를 복구했다는 주장은 구분한다.

문자열 결과는 quote 표시를 저장된 raw 본문과 구분한다. native result serializer는 String 본문을 실행 가능한 수식으로 읽지 않는다. unknown metadata·잘못된 XML/예산·index 설정을 명시적 경계로 남긴다. 원본 서식·전체 문자/Unicode·clipboard·num2str와 string matrix/CAS는 여전히 미완료다. 검사 개수나 입력 microbenchmark를 완성률로 환산하지 않는다.

최종 r202 검증은 [0186](0186-string-cache-and-plot-source.md)을 따른다. Windows/peets 각각1541검사(workspace+backend), 122MCDX와 별도10XML의 XSD 742parts·60PDF 구조 검증을 통과했다. r200 일반 검사와 잘못된 설정 fixture의 과거 반복 결과를 원본 설정 호환 근거로 사용하지 않는다.
