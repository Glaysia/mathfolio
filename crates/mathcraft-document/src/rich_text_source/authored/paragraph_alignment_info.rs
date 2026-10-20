use super::*;
impl AuthoredRichText {
    /// The generated LTR policy is available only after closed scaffold proof.
    pub fn paragraph_alignment_info(
        &self,
    ) -> Result<super::super::paragraph_alignment::ParagraphAlignmentInfo> {
        super::super::paragraph_alignment::alignment_info::source_info(
            self.xaml(),
            &self.index,
            true,
        )
    }
}
