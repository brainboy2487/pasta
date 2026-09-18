/* training.h - Simple training pipeline API
 * Provides a small C entrypoint to run the training pipeline steps:
 *  - build vocab from input
 *  - (optionally) generate MPHF artifact
 *  - invoke the existing training function to ingest data
 */
#ifndef MAGE_TRAINING_H
#define MAGE_TRAINING_H

#ifdef __cplusplus
extern "C" {
#endif

/* Run a minimal training pipeline for `input_path`.
 * Returns >=0 number of entries added, or negative error code.
 */
int mage_training_run_pipeline(const char* input_path);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_TRAINING_H */
