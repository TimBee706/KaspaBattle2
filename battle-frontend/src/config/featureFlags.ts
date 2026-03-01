/**
 * KaspaBattle Feature Flags
 * Use these to gate features for different rollout steps.
 */
export const FEATURE_FLAGS = {
    // STEP 1 - TEST MODE: Only require Kaspa wallet for challenges
    TEST_MODE: true,

    // STEP 2 - PRODUCTION: Require FaceID/Identity verification
    REQUIRE_FACE_ID: false,

    // Future: Require KYC
    REQUIRE_KYC: false,
} as const;
