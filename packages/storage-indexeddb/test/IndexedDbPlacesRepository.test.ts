import { sharedRepositoryContractTests } from "@terra-globe/core/src/storage/repositoryContractTests.js";
import { IndexedDbPlacesRepository } from "../src/IndexedDbPlacesRepository.js";

let dbCounter = 0;

sharedRepositoryContractTests(
  () => new IndexedDbPlacesRepository(`terra-globe-test-${dbCounter++}`),
);
