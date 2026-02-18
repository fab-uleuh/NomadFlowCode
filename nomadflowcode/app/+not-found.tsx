import { Link, Stack } from 'expo-router';
import { View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Text } from '@/components/ui/text';

export default function NotFoundScreen() {
  const { t } = useTranslation();

  return (
    <>
      <Stack.Screen options={{ title: t('common.error.not_found_title') }} />
      <View>
        <Text>{t('common.error.not_found_message')}</Text>

        <Link href="/">
          <Text>{t('common.error.go_home')}</Text>
        </Link>
      </View>
    </>
  );
}
