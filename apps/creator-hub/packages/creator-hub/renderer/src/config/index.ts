import { Env, createConfig } from '@dcl/ui-env';
import { env } from '#preload';
import dev from './env/dev.json';
import prod from './env/prd.json';
import stg from './env/stg.json';

export const config = createConfig(
  {
    [Env.DEVELOPMENT as string]: dev,
    [Env.STAGING as string]: stg,
    [Env.PRODUCTION as string]: prod,
  },
  {
    systemEnvVariables: {
      REACT_APP_DCL_DEFAULT_ENV: env.getEnv(),
    },
  },
);
