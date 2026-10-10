import { ProductStory as Shared } from '../../shared/site/ProductStory';
import { useSite } from '../../shared/site/runtime';
import { CopyCode } from './CopyCode';
export function ProductStory() {
  const { t } = useSite();
  return <Shared code={<CopyCode label={t('source.story.title.0')} code={'const customer = session.customer;\nreturn charge(customer.id, total);\n}'} />} />;
}
