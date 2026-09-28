import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { SectionHeading } from '../common/SectionHeading';
import { RecruitCard } from './RecruitCard';
import { publicLinks } from '../../config/publicLinks';
import { PUBLIC_CONTACT_EMAIL } from '../../config/constants';

// "Help build KaspaBattle" — Playtest / Contribute / Share feedback cards.
// Anchor id="get-involved" is the scroll target for the hero's secondary CTA.
export function RecruitmentSection() {
    const { t } = useTranslation();
    const navigate = useNavigate();

    return (
        <section id="get-involved" className="py-20 md:py-24 border-t border-kaspa-border scroll-mt-24">
            <SectionHeading title={t('sections.recruit.heading')} subtitle={t('sections.recruit.intro')} />
            <div className="grid grid-cols-1 md:grid-cols-3 gap-6 max-w-6xl mx-auto px-4">
                <RecruitCard
                    icon="wallet"
                    title={t('sections.recruit.playtest.title')}
                    audience={t('sections.recruit.playtest.audience')}
                    text={t('sections.recruit.playtest.text')}
                    ctas={[{ label: t('sections.recruit.playtest.cta'), onClick: () => navigate('/lobby') }]}
                />
                <RecruitCard
                    icon="code"
                    title={t('sections.recruit.contribute.title')}
                    audience={t('sections.recruit.contribute.audience')}
                    text={t('sections.recruit.contribute.text')}
                    ctas={[{ label: t('sections.recruit.contribute.cta'), href: publicLinks.githubRepository, external: true }]}
                />
                <RecruitCard
                    icon="bug"
                    title={t('sections.recruit.feedback.title')}
                    audience={t('sections.recruit.feedback.audience')}
                    text={t('sections.recruit.feedback.text')}
                    ctas={[
                        { label: t('sections.recruit.feedback.cta_issue'), href: publicLinks.githubIssueNew, external: true },
                        ...(PUBLIC_CONTACT_EMAIL
                            ? [{ label: t('sections.recruit.feedback.cta_contact'), href: `mailto:${PUBLIC_CONTACT_EMAIL}` }]
                            : []),
                    ]}
                />
            </div>
        </section>
    );
}
