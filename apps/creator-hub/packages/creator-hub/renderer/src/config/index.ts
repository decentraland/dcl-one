import { Env, createConfig } from '@dcl/ui-env';
import { env } from '#preload';
import { Env as AppEnv } from '/shared/types/env';
import dev from './env/dev.json';
import prod from './env/prd.json';
import dclOne from './env/dcl-one.json';

// @dcl/ui-env only accepts 'dev', 'stg' and 'prod', so the app's `dcl-one`
// environment is loaded through ui-env's staging slot.
const appEnv = env.getEnv();
const uiEnv = appEnv === AppEnv.DCL_ONE ? Env.STAGING : appEnv;

export const config = createConfig(
  {
    [Env.DEVELOPMENT as string]: dev,
    [Env.STAGING as string]: dclOne,
    [Env.PRODUCTION as string]: prod,
  },
  {
    systemEnvVariables: {
      REACT_APP_DCL_DEFAULT_ENV: uiEnv,
    },
  },
);
