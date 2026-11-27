# Shop web avatar preview entry

- Status: Accepted
- Date: 2026-11-26

The web Shop embeds an avatar preview iframe in item details, outfits and home promotions. The Explorer hosts the in-world preview and includes the shared Shop root in its input manifest.

The web Shop imports the iframe presentation through `@dcl/one-ui/apps/shop/preview`. The root entry stays independent of this module so the web iframe does not become an input to the HUD. The package export and directory inventory remain inputs to module resolution. Regenerate the HUD manifest when either changes and check the shared source and edge lists for preview modules.

The app supplies the device capabilities, feature flags, session profile and router link. Pure renderer selection lives in `library/shop/detail/lib`. The existing Explorer host chooses and supplies its own rendering surface.
