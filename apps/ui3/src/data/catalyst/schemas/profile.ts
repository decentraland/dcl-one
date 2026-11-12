
import { z } from "zod";

const Color3Schema = z
  .object({ r: z.number(), g: z.number(), b: z.number() })
  .partial()
  .passthrough();

const nullableStr = z.string().nullish();

// Lambdas sends these unset Avatar fields as an explicit null;
// reading null as undefined keeps their optional types. The key stays present.
const absentWhenNull = <T extends z.ZodType>(schema: T) =>
  schema.nullish().transform((v) => v ?? undefined).optional();

const LinkSchema = z.object({
  title: nullableStr,
  url: nullableStr,
});

export type ProfileLinkWire = z.infer<typeof LinkSchema>;

const AvatarInfoSchema = z
  .object({
    bodyShape: nullableStr,
    skin: z.object({ color: Color3Schema.required() }).nullish(),
    hair: z.object({ color: Color3Schema.required() }).nullish(),
    eyes: z.object({ color: Color3Schema.required() }).nullish(),
    wearables: z.array(z.string()).nullish(),
    snapshots: absentWhenNull(
      z.object({ face256: z.string().optional(), body: z.string().optional() }).passthrough(),
    ),
  })
  .passthrough();

export type AvatarInfoWire = z.infer<typeof AvatarInfoSchema>;

export const AvatarSchema = z
  .object({
    name: nullableStr,
    hasClaimedName: z.boolean().nullish(),
    nameColor: absentWhenNull(z.union([Color3Schema, z.string()])),
    description: nullableStr,
    links: absentWhenNull(z.array(LinkSchema)),
    country: nullableStr,
    gender: nullableStr,
    pronouns: nullableStr,
    relationshipStatus: nullableStr,
    sexualOrientation: nullableStr,
    language: nullableStr,
    profession: nullableStr,
    birthdate: z.number().nullish(),
    realName: nullableStr,
    hobbies: nullableStr,
    ethAddress: z.string().optional(),
    userId: z.string().optional(),
    avatar: AvatarInfoSchema.optional(),
  })
  .passthrough();

export type AvatarWire = z.infer<typeof AvatarSchema>;

export const ProfileEnvelopeSchema = z
  .object({
    avatars: z.array(AvatarSchema),
    timestamp: z.number().optional(),
  })
  .passthrough();

export type ProfileEnvelopeWire = z.infer<typeof ProfileEnvelopeSchema>;

const BadgeDataSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    description: z.string().nullish(),
    category: z.string().nullish(),
    isTier: z.boolean().optional(),
    completedAt: z.string().nullish(),
    assets: z.unknown().optional(),
    progress: z
      .object({
        stepsDone: z.number().optional(),
        totalStepsTarget: z.number().optional(),
        lastCompletedTierName: z.string().nullish(),
        lastCompletedTierImage: z.string().nullish(),
      })
      .passthrough()
      .optional(),
  })
  .passthrough();

export type BadgeData = z.infer<typeof BadgeDataSchema>;

export const UserBadgesEnvelopeSchema = z
  .object({
    data: z
      .object({
        achieved: z.array(BadgeDataSchema),
        notAchieved: z.array(BadgeDataSchema),
      })
      .passthrough(),
  })
  .passthrough();

const GalleryImageSchema = z
  .object({
    id: z.string(),
    url: z.string(),
    thumbnailUrl: z.string(),
    isPublic: z.boolean().optional(),
    dateTime: z.string(),
  })
  .passthrough();

export type GalleryImage = z.infer<typeof GalleryImageSchema>;

export const GalleryEnvelopeSchema = z
  .object({
    images: z.array(GalleryImageSchema),
    userData: z
      .object({
        currentImages: z.number().optional(),
        maxImages: z.number().optional(),
      })
      .passthrough()
      .optional(),
  })
  .passthrough();
