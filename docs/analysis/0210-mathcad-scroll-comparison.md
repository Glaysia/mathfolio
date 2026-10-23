# Mathcad가 같은 Windows에서 부드럽다는 비교와 판정 정정

2026-10-23. 사용자가 같은 Windows에서 원본 Mathcad는 스크롤 지연이 없다고 확인했다. 이 비교는 성공 기준이다. [0209](0209-windows-native-software-rendering.md)의 native 측정은 Mathcraft의 느린 경로를 관측했지만, GPU 부재를 근본 원인으로 확정하거나 렌더러 교체를 유일한 해법으로 정할 근거는 아니다.

## 원본의 정적 REA

`Ptc.Controls.Worksheet.dll`12.0.0.1, SHA256 `c1a70ec8007228ff119d4a31b36b9062132be3a10c771459c1dbe3fd0a7baddd`, MVID `44f31187-dcfc-4afd-a7d6-50fbee8dc361`의 실제 CIL body·호출 관계를 확인했다. 원본 코드를 실행하거나 CLR로 로드하지 않았다.

- `WorksheetControl.UpdateWorksheetLayoutOnScrollChanged` (`0x060004d3`,33byte IL)는 페이지 관리자의 visible-pages 갱신 결과를 받아 visible-pages control의 위쪽 padding을 갱신한다. WPF `Control.Padding` 호출을 확인했다.
- `WorksheetVisiblePages.UpdateVisiblePages` (`0x06000bfc`,298byte IL)는 기존 첫/마지막 페이지 인덱스와 새 범위를 비교한다. 같으면 IL offset190으로 분기해 범위 차이 처리 구간을 건너뛴다. 다르면 앞/뒤에 추가·제거하는 네 경로를 사용한다.
- 위 메서드의 호출 대상은 `AddToEnd`, `AddToBeginning`, `RemoveFromEnd`, `RemoveFromBeginning`이다. 스크롤마다 visible-pages 컬렉션을 무조건 전체 재생성하는 구조가 아니라 범위 차이에 따라 갱신하는 구조라는 근거다.

보존 Evidence: Worksheet 전체 metadata `ev_8bc4a418673e25c4b7d5b9fd1a090724d3f71cc9220c5088725d4f14966231b8`; 페이지 관리자 선택 `ev_4254b186d1d414f9e120747c8425940fd83936fec53803f39b751c2c953408db`; scroll/visible-pages 선택 `ev_151f745e1d99c2d0e37c97139c09a32ab2e0e44c7a3ae41bab00e5554c40d0db`.

정적 호출·분기로 확인한 범위다. WPF의 하부 픽셀 캐시, 실제 invalidation 범위, Mathcad의 선택 어댑터·CPU/GPU 시간, 해당 실행에서 호출된 횟수까지 확인한 것은 아니다. visible-pages 차이 갱신만으로 원본이 빠른 이유를 모두 설명하지 않는다. 우리 앱에도 이미 가시 영역 처리·조판 캐시가 있으므로, 이름이 비슷한 캐시를 추가하는 것으로 완료를 판정하지 않는다.

## 다음 비교의 기준

같은 문서·창 크기·확대·이동에서 원본과 Mathcraft의 실제 화면 갱신 비용을 비교한다. 조판된 내용의 재사용, 변경 영역의 처리, mesh/texture 전송과 raster·제출·표시를 구분한다. WGPU 유지·그리기 작업 축소·페이지 surface 재사용·다른2D backend는 확인할 후보이며 현재 확정된 해법이 아니다.

GPU가 없는 환경에서도 원본과 비슷한 반응성을 달성해야 한다. OS/VM 탓으로 완료 처리하지 않는다. 이번 변경은 분석·판정 정정이며 제품 스크롤 수정은 미완료다. 원본 DLL·CIL·리소스는 제품에 포함하지 않는다. 원자료는 로컬 `artifacts/rea/mathcad-scroll-comparison-20261023/evidence.json`에 보존한다.
