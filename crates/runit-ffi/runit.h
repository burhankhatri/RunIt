#ifndef RUNIT_H
#define RUNIT_H
#include <stddef.h>
typedef struct RunItApp RunItApp;
typedef void (*RunItCallback)(void *context, const char *event_json);
RunItApp *runit_open(const char *engine_dir, RunItCallback callback, void *context);
char *runit_call(const RunItApp *app, const char *method, const char *params_json);
void runit_free(char *string);
void runit_close(RunItApp *app);
#endif
