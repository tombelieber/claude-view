import type { ConfigContext, ExpoConfig } from 'expo/config'

export default ({ config }: ConfigContext): ExpoConfig => ({
  ...config,
  name: 'Claude View',
  slug: 'claude-view',
  version: '0.1.0',
  orientation: 'portrait',
  icon: './assets/images/icon.png',
  scheme: 'claude-view',
  userInterfaceStyle: 'automatic',
  splash: {
    image: './assets/images/splash-icon.png',
    resizeMode: 'contain',
    backgroundColor: '#ffffff',
  },
  ios: {
    supportsTablet: true,
    bundleIdentifier: 'ai.claudeview.mobile',
    associatedDomains: ['applinks:m.claudeview.ai'],
    infoPlist: {
      ITSAppUsesNonExemptEncryption: false,
    },
  },
  android: {
    package: 'ai.claudeview.mobile',
    adaptiveIcon: {
      foregroundImage: './assets/images/adaptive-icon.png',
      backgroundColor: '#ffffff',
    },
    intentFilters: [
      {
        action: 'VIEW',
        autoVerify: true,
        data: [{ scheme: 'https', host: 'm.claudeview.ai', pathPrefix: '/' }],
        category: ['BROWSABLE', 'DEFAULT'],
      },
    ],
  },
  web: {
    bundler: 'metro',
    output: 'static',
    favicon: './assets/images/favicon.png',
  },
  plugins: [
    'expo-router',
    'expo-secure-store',
    ['expo-camera', { cameraPermission: 'Allow Claude View to scan QR codes for pairing.' }],
    [
      'onesignal-expo-plugin',
      {
        mode: 'development',
      },
    ],
  ],
  extra: {
    eas: {
      projectId: 'f395dbf3-420b-4f67-8892-d466bd185d85',
    },
    oneSignalAppId: process.env.ONESIGNAL_APP_ID || 'YOUR_ONESIGNAL_APP_ID',
  },
  experiments: {
    typedRoutes: true,
  },
})
