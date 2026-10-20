# XY 단위·배율 통합 검증

2026-10-19. [0176 구현](0176-xy-physical-units-implementation.md)과 [0177 복원 context](0177-cold-native-context-restoration.md)의 고정 source r180을 Windows와 peets에서 검사했다. 원본 GUI·스크린샷·CLR/COM·Mathcad runtime 실행은 없다. r172 이력은 [0172](0172-xy-geometry-slot-integration.md)에 보존하며 테스트 수를 완성률로 환산하지 않는다.

같은 archive의 Windows workspace1407+backend2와 peets1409가 통과했다. 실패0, 양쪽 ignored78. fmt·workspace/backend Clippy `-D warnings`·build·reference38saved/38input·Windows release를 확인했다. peets는 모든462 source 파일의 SHA를 검증한 독립 checkout에서 실행했다.

양 OS의 **82 MCDX/410 XML part와 별도10 XML**, 합계420 XML 검사가 원본 XSD 오류0이다. 별도 XML은 typed scaling 도움말 구조3개와 unit codec2개씩이다. **54 PDF**는 image object0이며 fitz/pypdf 두 reader에서 텍스트를 추출했다. 단위 예제의 m/in, 자동 meter·meter² residual, 사용자 source와 결과 unitMonomial 분리, native Pending 상태도 검사했다. 이 검증은 원본 앱에서 생성 파일을 열었다는 뜻이 아니다.

물리량 통합은 X=[1,2,3]m, Y=x²에 자동 SI 표시와 X=in/Y=m²를 각각 계산한다. own v3/native cold open은 계산0, native 복사의 worksheet/result XML bytes는 동일하다. 실제 Tab3회로 X배율에 진입해 in→m을 입력하는 동안 document/curve/history/generation은 그대로이며 Enter가 한 Undo 명령이다. 이전 곡선은 Pending으로 저장·재열기하고 명시 계산 뒤 [2,4]로 바뀐다. PDF는 source scaling과 residual units를 구조 수식으로 그린다.

normal 회귀는 scalar/vector/range physical samples·pointwise 제곱 차원·UNIT/VARIABLE shadow·0.1m 대 dimensionless0.1·공유 X와 독립 Y 오류·잘못된 배율/PH·v1/v2 bytes/v3 strict source association·unknown native canonical 보존을 검사한다. typed editor는 UNIT default·local Undo·Ctrl+- literal suffix·Ctrl+Q·IME·Tab 외부 이탈을 확인한다. own cache 없는 보존 archive는 upstream 수정 시 Math/XY/Area 모두 Pending이며 다시 저장해도 Sync로 돌아가지 않는다. 실제 fresh native는 추가 context projection0회, own archive fallback은1회다. 원래 A4 f64와 importer f32의 **알려진 정확한 표현**을 모두 허용하며 가까운 임의 값은 거절한다.

남은 범위는 전체 단위 체계·derived 자동 선택·affine/complex 원본 정책·Call/binder 자동 label repair, 원본 sampling/domain 전체, Shift+Enter/삭제 슬롯 작성 키, legend/adaptive layout·큰 목록 overflow, 다른 plot 종류·원본 engine 오류/수명과 GUI/GPU 반응성이다. [0173](0173-xy-marker-tessellation-design.md)의 marker CPU 문제는 이 작업에서 해결하지 않았다. [0177](0177-cold-native-context-restoration.md)의4000식 own-no-cache 추가 proof 약154ms도 숨기지 않는다. 사용자 평가15%에서40% 목표는 계속 진행한다.

## 고정 산출물

archive `artifacts/rea/mathcraft-xy-units-20261019-r180.tar`는12,656,640bytes, SHA `a02a457a026c2cd0f99f6e5a9c2c81d8ebbbaf7f706021ef066d9f4be0f24ef3`다. manifest `artifacts/rea/xy-units-integration-20261019-r180/source-manifest.json` SHA `3ab386b47ced1a9eee71caf107860e18a99b0d0dcb8e721cd57229b1bc0c102a`,462files/222product Rust다. peets checkout은 `/home/peets/Projects/mathcraft-xy-units-20261019-r180`다.

회수 evidence `xy-units-r180-peets-evidence.tar`는2,775,040bytes, SHA `0ce794042f627312842ef9219e060bdfb65582d94d0418faf09231b5152b4d8b`로 양끝 같다. `artifacts/rea/xy-units-integration-20261019-r180/validation.json`은 전체 검사 및 finalization 시점의 current product=archive를 확인한다. source archive 당시 문서는 진행 기록일 수 있으며 본 최종 설명은 이후 갱신이다. 이후 제품 변경은 별도 revision으로 검사한다.

Windows release `mathcraft-xy-units-r180-20261019.exe`는24,296,960bytes, SHA `4e434e99427128f7fae07e835f72b569f4c08e351b31f17dae022f57f7108c79`다. headless harness26,357,760bytes, SHA `f204dc339323ced851436ab0b4871d1861e8d51cc891a06fce15517ef8c6cc07`다. release는r172보다576,512bytes 증가했다. 새 화면 성능 개선 수치로 해석하지 않는다.
